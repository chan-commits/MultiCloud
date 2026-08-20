use anyhow::{Context, bail};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input, Password, Select, theme::ColorfulTheme};
use multicloud_authorization::system_role_specs;
use multicloud_configuration::Settings;
use multicloud_identity::Email;
use multicloud_operation::EventEnvelope;
use multicloud_persistence::{
    entities::{
        organization_memberships, organizations, permissions, role_bindings, role_permissions,
        roles, sessions, users,
    },
    reliable_events::enqueue_event,
};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, Set, Statement, TransactionTrait,
    sea_query::OnConflict,
};
use std::{collections::HashMap, io::IsTerminal, path::PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "multicloud", about = "MultiCloud local administration utility")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create the first administrator and organization.
    Init {
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        organization_slug: Option<String>,
        #[arg(long)]
        organization_name: Option<String>,
    },
    /// Reset a user's password and restore owner access for one organization.
    RecoverAccess {
        email: Option<String>,
        /// Organization UUID. Required only when no existing membership can be selected.
        #[arg(long)]
        organization: Option<Uuid>,
    },
}

#[allow(clippy::missing_errors_doc)]
pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    assert_interactive_terminal()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let settings = Settings::load(root).context("could not load settings")?;
    let database = multicloud_persistence::connect(
        &settings.database.url,
        settings.database.max_connections.min(2),
    )
    .await
    .context("could not connect to database; run migrations first")?;

    match cli.command {
        Command::Init {
            email,
            display_name,
            organization_slug,
            organization_name,
        } => {
            initialize(
                &database,
                email,
                display_name,
                organization_slug,
                organization_name,
            )
            .await
        }
        Command::RecoverAccess {
            email,
            organization,
        } => recover_access(&database, email, organization).await,
    }
}

fn assert_interactive_terminal() -> anyhow::Result<()> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!(
            "this command requires an interactive terminal so secrets cannot be passed as arguments"
        );
    }
    Ok(())
}

async fn initialize(
    database: &DatabaseConnection,
    email: Option<String>,
    display_name: Option<String>,
    organization_slug: Option<String>,
    organization_name: Option<String>,
) -> anyhow::Result<()> {
    if users::Entity::find().one(database).await?.is_some() {
        bail!("MultiCloud is already initialized; use recover-access for an existing user");
    }
    let theme = ColorfulTheme::default();
    let email = normalized_email(prompt_or(email, "Administrator email", &theme)?)?;
    let display_name_input = prompt_or(display_name, "Administrator display name", &theme)?;
    let display_name = nonempty(&display_name_input, "display name", 120)?;
    let slug = prompt_or(organization_slug, "Organization slug", &theme)?
        .trim()
        .to_ascii_lowercase();
    if !valid_slug(&slug) {
        bail!("organization slug must be 3-80 lowercase letters, digits, or internal hyphens");
    }
    let organization_name_input = prompt_or(organization_name, "Organization name", &theme)?;
    let organization_name = nonempty(&organization_name_input, "organization name", 160)?;
    let password_hash = prompt_password(&format!("Password for {email}"), &theme)?;

    println!("\nAdministrator: {email}\nOrganization:  {organization_name} ({slug})");
    if !Confirm::with_theme(&theme)
        .with_prompt("Initialize MultiCloud with this administrator?")
        .default(true)
        .interact()?
    {
        println!("Initialization cancelled.");
        return Ok(());
    }

    let user_id = Uuid::now_v7();
    let organization_id = Uuid::now_v7();
    let now = OffsetDateTime::now_utc();
    let transaction = database.begin().await?;
    users::ActiveModel {
        id: Set(user_id),
        email: Set(email.clone()),
        display_name: Set(display_name),
        status: Set("active".to_owned()),
        password_hash: Set(password_hash),
        is_platform_admin: Set(true),
        email_verified_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&transaction)
    .await?;
    set_tenant_context(&transaction, user_id, Some(organization_id)).await?;
    organizations::ActiveModel {
        id: Set(organization_id),
        slug: Set(slug),
        name: Set(organization_name),
        status: Set("active".to_owned()),
        settings: Set(serde_json::json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
    }
    .insert(&transaction)
    .await?;
    upsert_membership(&transaction, organization_id, user_id, now).await?;
    ensure_system_roles(&transaction, organization_id, user_id, now).await?;
    enqueue_admin_event(
        &transaction,
        organization_id,
        user_id,
        "identity.bootstrap.completed",
        serde_json::json!({ "email": email, "access": "owner", "source": "local_cli" }),
    )
    .await?;
    transaction.commit().await?;

    println!("MultiCloud initialized. You can now sign in as {email}.");
    Ok(())
}

async fn recover_access(
    database: &DatabaseConnection,
    email: Option<String>,
    explicit_organization: Option<Uuid>,
) -> anyhow::Result<()> {
    let theme = ColorfulTheme::default();
    let user = select_user(database, email, &theme).await?;
    let selection_transaction = database.begin().await?;
    set_tenant_context(&selection_transaction, user.id, None).await?;
    let memberships = organization_memberships::Entity::find()
        .filter(organization_memberships::Column::UserId.eq(user.id))
        .order_by_asc(organization_memberships::Column::CreatedAt)
        .all(&selection_transaction)
        .await?;
    let organization_id = select_organization(
        &selection_transaction,
        user.id,
        &memberships,
        explicit_organization,
        &theme,
    )
    .await?;
    set_tenant_context(&selection_transaction, user.id, Some(organization_id)).await?;
    let organization = organizations::Entity::find_by_id(organization_id)
        .one(&selection_transaction)
        .await?
        .context("organization was not found or cannot be accessed")?;
    selection_transaction.rollback().await?;
    let password_hash = prompt_password(&format!("New password for {}", user.email), &theme)?;

    println!(
        "\nUser:         {}\nOrganization: {} ({})\n\nThis will reset the password, revoke all sessions, reactivate the user and membership, and ensure Owner access.",
        user.email, organization.name, organization.slug
    );
    if !Confirm::with_theme(&theme)
        .with_prompt("Continue with access recovery?")
        .default(false)
        .interact()?
    {
        println!("Access recovery cancelled.");
        return Ok(());
    }

    let now = OffsetDateTime::now_utc();
    let user_id = user.id;
    let transaction = database.begin().await?;
    set_tenant_context(&transaction, user_id, Some(organization_id)).await?;
    let mut active_user: users::ActiveModel = user.into();
    active_user.password_hash = Set(password_hash);
    active_user.status = Set("active".to_owned());
    active_user.updated_at = Set(now);
    active_user.update(&transaction).await?;
    sessions::Entity::update_many()
        .col_expr(
            sessions::Column::RevokedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(sessions::Column::UserId.eq(user_id))
        .filter(sessions::Column::RevokedAt.is_null())
        .exec(&transaction)
        .await?;
    upsert_membership(&transaction, organization_id, user_id, now).await?;
    ensure_system_roles(&transaction, organization_id, user_id, now).await?;
    enqueue_admin_event(
        &transaction,
        organization_id,
        user_id,
        "identity.access.recovered",
        serde_json::json!({
            "account_id": user_id,
            "organization_id": organization_id,
            "sessions_revoked": true,
            "access": "owner",
            "source": "local_cli"
        }),
    )
    .await?;
    transaction.commit().await?;
    println!("Access recovered. All previous sessions were revoked.");
    Ok(())
}

async fn select_user(
    database: &DatabaseConnection,
    email: Option<String>,
    theme: &ColorfulTheme,
) -> anyhow::Result<users::Model> {
    if let Some(email) = email {
        let email = normalized_email(email)?;
        return users::Entity::find()
            .filter(users::Column::Email.eq(email))
            .one(database)
            .await?
            .context("user was not found");
    }
    let users = users::Entity::find()
        .order_by_asc(users::Column::Email)
        .all(database)
        .await?;
    if users.is_empty() {
        bail!("no users exist; run init first");
    }
    let labels: Vec<_> = users
        .iter()
        .map(|user| format!("{} ({})", user.email, user.display_name))
        .collect();
    let selected = Select::with_theme(theme)
        .with_prompt("Select a user to recover")
        .items(&labels)
        .default(0)
        .interact()?;
    users
        .into_iter()
        .nth(selected)
        .context("invalid user selection")
}

async fn select_organization(
    transaction: &DatabaseTransaction,
    user_id: Uuid,
    memberships: &[organization_memberships::Model],
    explicit: Option<Uuid>,
    theme: &ColorfulTheme,
) -> anyhow::Result<Uuid> {
    if let Some(id) = explicit {
        return Ok(id);
    }
    if memberships.is_empty() {
        bail!(
            "the user has no membership; rerun with --organization <UUID> to explicitly grant access"
        );
    }
    if memberships.len() == 1 {
        return Ok(memberships[0].organization_id);
    }
    let mut labels = Vec::with_capacity(memberships.len());
    for membership in memberships {
        set_tenant_context(transaction, user_id, Some(membership.organization_id)).await?;
        let organization = organizations::Entity::find_by_id(membership.organization_id)
            .one(transaction)
            .await?;
        labels.push(organization.map_or_else(
            || membership.organization_id.to_string(),
            |organization| format!("{} ({})", organization.name, organization.slug),
        ));
    }
    let selected = Select::with_theme(theme)
        .with_prompt("Select the organization where Owner access will be restored")
        .items(&labels)
        .default(0)
        .interact()?;
    Ok(memberships[selected].organization_id)
}

async fn upsert_membership(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    user_id: Uuid,
    now: OffsetDateTime,
) -> anyhow::Result<()> {
    organization_memberships::Entity::insert(organization_memberships::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(organization_id),
        user_id: Set(user_id),
        status: Set("active".to_owned()),
        joined_at: Set(now),
        created_at: Set(now),
        updated_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([
            organization_memberships::Column::OrganizationId,
            organization_memberships::Column::UserId,
        ])
        .update_columns([
            organization_memberships::Column::Status,
            organization_memberships::Column::UpdatedAt,
        ])
        .to_owned(),
    )
    .exec_without_returning(transaction)
    .await?;
    Ok(())
}

async fn ensure_system_roles(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    owner_id: Uuid,
    now: OffsetDateTime,
) -> anyhow::Result<()> {
    let catalog = permissions::Entity::find().all(transaction).await?;
    let permission_by_key: HashMap<_, _> = catalog
        .iter()
        .map(|permission| (permission.key.as_str(), permission.id))
        .collect();
    let mut owner_role_id = None;
    for spec in system_role_specs() {
        let existing = roles::Entity::find()
            .filter(roles::Column::OrganizationId.eq(organization_id))
            .filter(roles::Column::Key.eq(spec.key))
            .one(transaction)
            .await?;
        let role_id = if let Some(role) = existing {
            role.id
        } else {
            let role_id = Uuid::now_v7();
            roles::ActiveModel {
                id: Set(role_id),
                organization_id: Set(organization_id),
                key: Set(spec.key.to_owned()),
                name: Set(spec.name.to_owned()),
                description: Set(spec.description.to_owned()),
                is_system: Set(true),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(transaction)
            .await?;
            role_id
        };
        for permission_key in spec.permission_keys {
            let permission_id = permission_by_key
                .get(permission_key)
                .copied()
                .with_context(|| format!("permission catalog is missing {permission_key}"))?;
            role_permissions::Entity::insert(role_permissions::ActiveModel {
                role_id: Set(role_id),
                permission_id: Set(permission_id),
                organization_id: Set(organization_id),
            })
            .on_conflict_do_nothing()
            .exec_without_returning(transaction)
            .await?;
        }
        if spec.key == "owner" {
            owner_role_id = Some(role_id);
        }
    }
    let owner_role_id = owner_role_id.context("owner role could not be created")?;
    role_bindings::Entity::insert(role_bindings::ActiveModel {
        id: Set(Uuid::now_v7()),
        organization_id: Set(organization_id),
        role_id: Set(owner_role_id),
        subject_type: Set("user".to_owned()),
        subject_id: Set(owner_id),
        scope_type: Set("organization".to_owned()),
        scope_id: Set(organization_id),
        created_by: Set(owner_id),
        created_at: Set(now),
    })
    .on_conflict_do_nothing()
    .exec_without_returning(transaction)
    .await?;
    Ok(())
}

async fn enqueue_admin_event(
    transaction: &DatabaseTransaction,
    organization_id: Uuid,
    user_id: Uuid,
    event_type: &str,
    payload: serde_json::Value,
) -> anyhow::Result<()> {
    enqueue_event(
        transaction,
        EventEnvelope {
            id: multicloud_shared_kernel::EventId::new(),
            organization_id: multicloud_shared_kernel::OrganizationId::from_uuid(organization_id),
            aggregate_type: "user".to_owned(),
            aggregate_id: user_id.to_string(),
            event_type: event_type.to_owned(),
            event_version: 1,
            payload,
            trace_id: None,
            occurred_at: OffsetDateTime::now_utc(),
        },
    )
    .await?;
    Ok(())
}

async fn set_tenant_context(
    transaction: &DatabaseTransaction,
    user_id: Uuid,
    organization_id: Option<Uuid>,
) -> anyhow::Result<()> {
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT set_config('app.user_id', $1, true), set_config('app.organization_id', $2, true)",
            [
                user_id.to_string().into(),
                organization_id.map_or_else(String::new, |id| id.to_string()).into(),
            ],
        ))
        .await?;
    Ok(())
}

fn prompt_or(value: Option<String>, prompt: &str, theme: &ColorfulTheme) -> anyhow::Result<String> {
    value.map_or_else(
        || {
            Input::<String>::with_theme(theme)
                .with_prompt(prompt)
                .interact_text()
                .map_err(Into::into)
        },
        Ok,
    )
}

fn prompt_password(prompt: &str, theme: &ColorfulTheme) -> anyhow::Result<String> {
    let password = Password::with_theme(theme)
        .with_prompt(prompt)
        .with_confirmation("Confirm password", "Passwords do not match")
        .interact()?;
    if password.chars().count() < 12 {
        bail!("password must contain at least 12 characters");
    }
    let mut salt_bytes = [0_u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|error| anyhow::anyhow!("could not generate password salt: {error}"))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow::anyhow!("could not hash password: {error}"))
}

fn normalized_email(value: String) -> anyhow::Result<String> {
    Email::parse(value)
        .map(|email| email.to_string())
        .map_err(|_| anyhow::anyhow!("invalid email address"))
}

fn nonempty(value: &str, field: &str, max: usize) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > max {
        bail!("{field} must contain 1-{max} characters");
    }
    Ok(value.to_owned())
}

fn valid_slug(value: &str) -> bool {
    let bytes = value.as_bytes();
    (3..=80).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}
