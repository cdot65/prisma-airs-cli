import React, {type ReactNode} from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import useBaseUrl from '@docusaurus/useBaseUrl';
import styles from './index.module.css';
const paths = [["01", "Run your first command", "Install the CLI, configure a tenant, and verify your connection.", "/getting-started/quick-start"], ["02", "Manage AI Gateway", "Follow workspace, provider, routing, and policy workflows.", "/cli/aigateway/workflows/"], ["03", "Find the right command", "Explore flags, output formats, and examples for every product group.", "/cli/"], ["04", "Automate your operations", "Use the library to compose repeatable workflows in your own tools.", "/developers/library/getting-started"]];
export default function Home(): ReactNode {
  return <Layout title="Prisma AIRS CLI" description="Operate Prisma AIRS with clear commands. Scan prompts, manage gateway resources, run red-team assessments, and bring repeatable security workflows to your team."><main>
    <section className={styles.hero} aria-labelledby="hero-title">
      <div><p className={styles.eyebrow}>PRISMA AIRS / CLI</p><h1 id="hero-title">Security at your fingertips.<br /><span>From one terminal.</span></h1>
      <p className={styles.lead}>Operate Prisma AIRS with clear commands. Scan prompts, manage gateway resources, run red-team assessments, and bring repeatable security workflows to your team.</p><div className={styles.actions}><Link className="button button--primary button--lg" to="/getting-started/installation">Get started →</Link><Link className={styles.secondary} to="/cli/">Command reference ↗</Link></div><p className={styles.platforms}>TENANT-AWARE · SCRIPTABLE · BUILT FOR OPERATORS</p></div>
      <div className={styles.artwork}><img src={useBaseUrl('/img/brand-logo.png')} alt="Prisma AIRS CLI shield and prism spectrum" width="1254" height="1254" fetchPriority="high" /><div className={styles.pillRow}><span className={styles.pill}>Secure by design</span><span className={styles.pill}>Developer first</span></div></div>
    </section>
    <section className={styles.paths} aria-labelledby="paths-title"><div className={styles.sectionIntro}><p className={styles.eyebrow}>BUILD · CONNECT · PROTECT · SCALE</p><h2 id="paths-title">Turn intent into action.</h2><p>Choose a starting point. Go from your first request to a repeatable security workflow.</p></div><div className={styles.grid}>{paths.map(([number,title,description,to]) => <Link className={styles.path} to={to} key={number}><span className={styles.number}>{number}</span><h3>{title}</h3><p>{description}</p><span className={styles.arrow} aria-hidden="true">↗</span></Link>)}</div></section>
    <section className={styles.quick}><div><p className={styles.eyebrow}>THE PRISMA AIRS TOOLKIT</p><h2>Build a safer tomorrow.</h2><p>Explore the platform's APIs, commands, and gateway-connected agent.</p></div><div className={styles.actions}><Link to="https://cdot65.github.io/prisma-airs-sdk/">SDK →</Link><Link to="https://cdot65.github.io/prisma-airs-cli/">CLI →</Link><Link to="https://cdot65.github.io/prisma-airs-harness/">Harness →</Link></div></section>
  </main></Layout>;
}
