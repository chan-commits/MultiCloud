import { error } from '@sveltejs/kit';
import { isView, viewFromRoute } from '$lib/navigation';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ params }) => {
  const route = params.view || undefined;
  if (route && !isView(route)) error(404, 'Page not found');
  return { view: viewFromRoute(route) };
};
