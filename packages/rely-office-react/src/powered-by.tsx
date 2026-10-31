// Copyright 2026 RelyBytes S.r.l. <https://relybytes.com>
// SPDX-License-Identifier: Apache-2.0

/**
 * The credit line at the bottom left of every editor. `poweredBy={false}` hides it, which is meant
 * for the hosts RelyBytes S.r.l. has agreed it with: see the package README.
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
