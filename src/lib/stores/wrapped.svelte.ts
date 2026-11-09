// Your year on the couch, as a story you tap through.
export const wrapped = $state({ open: false, year: new Date().getFullYear() });
export function openWrapped(year = new Date().getFullYear()) {
  wrapped.year = year;
  wrapped.open = true;
}
