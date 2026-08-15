import styles from "./EmptyPage.module.css";

// Routable and styled, with no content: OPTIONS will later own the airport file and the Aurora
// connection, both of which already have settings and connection-state APIs waiting for it.
export function OptionsPage() {
  return <div className={styles.page} />;
}
