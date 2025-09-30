import type { ReactNode } from "react";
import clsx from "clsx";
import Link from "@docusaurus/Link";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import Layout from "@theme/Layout";
import HomepageFeatures from "@site/src/components/HomepageFeatures";
import Heading from "@theme/Heading";

import styles from "./index.module.css";

export default function Home(): ReactNode {
  const { siteConfig } = useDocusaurusContext();
  return (
    <Layout
      title={`${siteConfig.title} - Vector Enhanced Search`}
      description="Understand your logs better"
    >
      <header className={styles.heroBanner}>
        <div className="styles.contentContainer">
          <h1 className="hero__title">
            VES is the open-source semantic log search tool for your cloud
            applications
          </h1>
          <p className="hero__subtitle">
            Search your application, system, kubernetes and docker logs
            semantically, in real-time. Log vector search and retrieval that is
            fast and affordable. Start locally or try it in the Cloud(Coming
            Soon)
          </p>
        </div>
      </header>
      <main>
        <HomepageFeatures />
      </main>
    </Layout>
  );
}
