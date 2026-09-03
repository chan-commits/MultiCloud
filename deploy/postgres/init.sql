CREATE ROLE multicloud
    LOGIN
    PASSWORD 'multicloud'
    NOSUPERUSER
    NOCREATEDB
    NOCREATEROLE
    NOINHERIT;

CREATE DATABASE multicloud OWNER multicloud;
