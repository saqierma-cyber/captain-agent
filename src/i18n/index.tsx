import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { type Locale, LOCALES, TRANSLATIONS } from "./translations";

const STORAGE_KEY = "captain-agent.locale";
const DEFAULT_LOCALE: Locale = "zh";

interface I18nCtx {
  locale: Locale;
  setLocale: (l: Locale) => void;
  t: (key: string, vars?: Record<string, string | number>) => string;
}

const Ctx = createContext<I18nCtx | null>(null);

function loadInitialLocale(): Locale {
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as Locale | null;
    if (saved && (saved in TRANSLATIONS)) return saved;
  } catch {
    /* ignore */
  }
  return DEFAULT_LOCALE;
}

function isRtl(loc: Locale) {
  return loc === "ar";
}

function applyDirToDom(loc: Locale) {
  const dir = isRtl(loc) ? "rtl" : "ltr";
  if (typeof document !== "undefined") {
    document.documentElement.lang = loc;
    document.documentElement.dir = dir;
  }
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => loadInitialLocale());

  useEffect(() => {
    applyDirToDom(locale);
  }, [locale]);

  const setLocale = useCallback((l: Locale) => {
    setLocaleState(l);
    try {
      localStorage.setItem(STORAGE_KEY, l);
    } catch {
      /* ignore */
    }
  }, []);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>): string => {
      const dict = TRANSLATIONS[locale] ?? TRANSLATIONS[DEFAULT_LOCALE];
      let text = dict[key] ?? TRANSLATIONS[DEFAULT_LOCALE][key] ?? key;
      if (vars) {
        for (const [k, v] of Object.entries(vars)) {
          // Use split/join for ES2015 compat (avoid String.replaceAll which
          // is ES2021+). The tsconfig target is unclear and we want zero-deps.
          text = text.split(`{${k}}`).join(String(v));
        }
      }
      return text;
    },
    [locale],
  );

  const value = useMemo<I18nCtx>(() => ({ locale, setLocale, t }), [locale, setLocale, t]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useI18n(): I18nCtx {
  const v = useContext(Ctx);
  if (!v) throw new Error("useI18n must be used within I18nProvider");
  return v;
}

export { LOCALES };
export type { Locale };
