import React, {type ReactNode} from 'react';
import {useDoc} from '@docusaurus/plugin-content-docs/client';
import ContentVisibility from '@theme/ContentVisibility';
import DocBreadcrumbs from '@theme/DocBreadcrumbs';
import DocItemContent from '@theme/DocItem/Content';
import DocItemFooter from '@theme/DocItem/Footer';
import type {Props} from '@theme/DocItem/Layout';
import DocItemPaginator from '@theme/DocItem/Paginator';
import DocItemTOCMobile from '@theme/DocItem/TOC/Mobile';
import DocVersionBadge from '@theme/DocVersionBadge';
import DocVersionBanner from '@theme/DocVersionBanner';
import styles from './styles.module.css';

export default function DocItemLayout({children}: Props): ReactNode {
  const {metadata, frontMatter, toc} = useDoc();
  return (
    <div className={styles.docItemContainer}>
      <ContentVisibility metadata={metadata} />
      <DocVersionBanner />
      <article>
        <DocBreadcrumbs />
        <DocVersionBadge />
        {!frontMatter.hide_table_of_contents && toc.length > 0 && <DocItemTOCMobile />}
        <DocItemContent>{children}</DocItemContent>
        <DocItemFooter />
      </article>
      <DocItemPaginator />
    </div>
  );
}
