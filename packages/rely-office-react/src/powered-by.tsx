// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0 AND LicenseRef-RelyBytes-Attribution

/**
 * The credit line at the bottom left of every editor. LICENSE-ATTRIBUTION keeps it visible:
 * `poweredBy={false}` hides it only for the hosts RelyBytes S.r.l. has allowed in writing.
 */
export function PoweredBy() {
  return (
    <div className="rely-powered-by">
      Powered by{' '}
      <a href="https://relybytes.com" target="_blank" rel="noreferrer">
        RelyBytes S.r.l.
      </a>
    </div>
  );
}
