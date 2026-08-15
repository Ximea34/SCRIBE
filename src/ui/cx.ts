type ClassName = string | false | null | undefined;

// CSS Module keys are `string | undefined` under noUncheckedIndexedAccess, so joining them
// needs one narrowing point rather than an assertion at every call site.
export function cx(...classes: ClassName[]): string {
  return classes.filter((value): value is string => Boolean(value)).join(" ");
}
