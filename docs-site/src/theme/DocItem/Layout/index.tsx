import React, { type ReactNode, useState } from 'react';
import { useDoc } from '@docusaurus/plugin-content-docs/client';
import { useWindowSize } from '@docusaurus/theme-common';
import clsx from 'clsx';
import ContentVisibility from '@theme/ContentVisibility';
import DocBreadcrumbs from '@theme/DocBreadcrumbs';
import DocItemContent from '@theme/DocItem/Content';
import DocItemFooter from '@theme/DocItem/Footer';
import type { Props } from '@theme/DocItem/Layout';
import DocItemPaginator from '@theme/DocItem/Paginator';
import DocItemTOCDesktop from '@theme/DocItem/TOC/Desktop';
import DocItemTOCMobile from '@theme/DocItem/TOC/Mobile';
import DocVersionBadge from '@theme/DocVersionBadge';
import DocVersionBanner from '@theme/DocVersionBanner';

import styles from './styles.module.css';

const ON_PAGE_NAVIGATION_ID = 'doc-page-navigation';

function useDocTOC() {
  const { frontMatter, toc } = useDoc();
  const windowSize = useWindowSize();
  const hidden = frontMatter.hide_table_of_contents;
  const canRender = !hidden && toc.length > 0;

  return {
    hidden,
    mobile: canRender ? <DocItemTOCMobile /> : undefined,
    desktop:
      canRender && (windowSize === 'desktop' || windowSize === 'ssr') ? (
        <DocItemTOCDesktop />
      ) : undefined,
  };
}

export default function DocItemLayout({ children }: Props): ReactNode {
  const docTOC = useDocTOC();
  const { metadata } = useDoc();
  const [tocCollapsed, setTocCollapsed] = useState(false);

  return (
    <div className="row">
      <div
        className={clsx(
          'col',
          !docTOC.hidden && styles.docItemCol,
          docTOC.desktop && tocCollapsed && styles.docItemColTocCollapsed,
        )}
      >
        <ContentVisibility metadata={metadata} />
        <DocVersionBanner />
        <div className={styles.docItemContainer}>
          <article>
            <DocBreadcrumbs />
            <DocVersionBadge />
            {docTOC.mobile}
            <DocItemContent>{children}</DocItemContent>
            <DocItemFooter />
          </article>
          <DocItemPaginator />
        </div>
      </div>
      {docTOC.desktop && (
        <aside
          className={clsx(styles.tocColumn, tocCollapsed && styles.tocColumnCollapsed)}
          aria-label="On-page navigation"
        >
          <div className={styles.tocPanel}>
            <button
              type="button"
              className={styles.tocToggle}
              aria-label={tocCollapsed ? 'Expand on-page navigation' : 'Collapse on-page navigation'}
              aria-controls={ON_PAGE_NAVIGATION_ID}
              aria-expanded={!tocCollapsed}
              title={tocCollapsed ? 'Expand on-page navigation' : 'Collapse on-page navigation'}
              onClick={() => setTocCollapsed((value) => !value)}
            >
              <span className={styles.tocToggleIcon} aria-hidden="true">
                {tocCollapsed ? '‹' : '›'}
              </span>
            </button>
            <div
              id={ON_PAGE_NAVIGATION_ID}
              className={styles.tocContent}
              hidden={tocCollapsed}
            >
              {docTOC.desktop}
            </div>
          </div>
        </aside>
      )}
    </div>
  );
}
