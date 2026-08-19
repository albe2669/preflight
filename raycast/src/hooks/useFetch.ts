/**
 * Minimal async-fetch hook with loading/error state. Raycast's @raycast/utils
 * has `useFetch`, but we hand-roll against our typed client so every view
 * shares the same empty/loading/error shape and the unreachable-detection.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { isUnreachable } from "../lib/graphql";

export interface FetchState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  unreachable: boolean;
  reload: () => void;
}

/**
 * Fetch `loader` on mount and whenever `reloadKey` changes. A manual `reload`
 * bumps an internal counter to re-run. The fetch is cancelled if the component
 * unmounts or a newer fetch starts (stale-result guard via a token).
 */
export function useFetch<T>(loader: () => Promise<T>, deps: unknown[] = []): FetchState<T> {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  const loaderRef = useRef(loader);
  loaderRef.current = loader;

  const reload = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    loaderRef
      .current()
      .then((result) => {
        if (cancelled) return;
        setData(result);
        setLoading(false);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        const msg = e instanceof Error ? e.message : String(e);
        setData(null);
        setError(msg);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tick, ...deps]);

  const unreachable = error != null && isUnreachable(error);
  return { data, loading, error, unreachable, reload };
}

/** Fetch several loaders at once; reload re-runs all. */
export function useFetchAll<T extends Record<string, unknown>>(
  loaders: () => Promise<T>,
  deps: unknown[] = [],
): FetchState<T> {
  return useFetch(loaders, deps);
}
