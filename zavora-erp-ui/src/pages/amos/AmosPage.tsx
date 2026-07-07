import { useEffect, useRef } from 'react';
import { getAccessToken } from '../../api/client';
import { getTimezone, getWorkDate } from '../../utils/workDate';

// Dev: the standalone Amos service. Prod: same-origin path proxied by Caddy
// (keeps the CDN/TLS story simple and lets the iframe inherit mic permission).
const AMOS_URL =
  import.meta.env.VITE_AMOS_URL || (import.meta.env.PROD ? '/amos-app' : 'http://localhost:8090');
// Absolute origin to target postMessage at (never '*' — the token is a bearer).
const AMOS_ORIGIN = new URL(AMOS_URL, window.location.origin).origin;

/**
 * Amos lives in its own service (:8090); embedding it keeps the ERP shell
 * (sidebar + header) so the experience feels like one system. `embed=1`
 * tells the Amos UI to hide its standalone header.
 *
 * Identity: the ERP holds the user's access token in memory. We hand it to the
 * Amos iframe via postMessage (targeted origin), and re-send periodically so
 * Amos's copy survives the ERP's silent token refresh. Amos verifies the token
 * and refuses any session whose entity ≠ the one it serves.
 */
export default function AmosPage() {
  const frameRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    const post = () => {
      const token = getAccessToken();
      if (token && frameRef.current?.contentWindow) {
        // Forward the user's timezone + work-as-of (posting) date so Amos
        // grounds "today" and default posting dates on the same preferences the
        // ERP forms use (see utils/workDate.ts).
        frameRef.current.contentWindow.postMessage(
          { type: 'amos-auth', token, timezone: getTimezone(), work_date: getWorkDate() },
          AMOS_ORIGIN,
        );
      }
    };
    // Post on iframe load and every 60s (tokens live ~15 min; keeps Amos fresh).
    const frame = frameRef.current;
    frame?.addEventListener('load', post);
    const id = window.setInterval(post, 60_000);
    // Amos may also request the token on connect.
    const onMsg = (e: MessageEvent) => {
      if (e.origin === AMOS_ORIGIN && e.data?.type === 'amos-auth-request') post();
    };
    window.addEventListener('message', onMsg);
    // Re-post when the user changes their work-date / timezone so Amos updates
    // mid-session.
    window.addEventListener('zavora:workdate-changed', post);
    return () => {
      frame?.removeEventListener('load', post);
      window.clearInterval(id);
      window.removeEventListener('message', onMsg);
      window.removeEventListener('zavora:workdate-changed', post);
    };
  }, []);

  return (
    <div className="-m-6 h-[calc(100vh-3.5rem)]">
      <iframe
        ref={frameRef}
        src={`${AMOS_URL}/?embed=1`}
        title="Amos — AI Accountant"
        className="w-full h-full border-0 block"
        allow="microphone; autoplay; clipboard-write"
      />
    </div>
  );
}
