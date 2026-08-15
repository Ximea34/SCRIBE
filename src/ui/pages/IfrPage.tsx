import styles from "./EmptyPage.module.css";

// Routable and styled, with no content: the IFR tab is out of scope for this build.
export function IfrPage() {
  return <div className={styles.page} />;
}
