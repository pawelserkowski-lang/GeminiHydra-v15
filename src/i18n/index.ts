// src/i18n/index.ts
/**
 * GeminiHydra v15 - i18next Configuration
 * ==========================================
 * EN/PL translations with English fallback.
 */

import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from './en.json';
import pl from './pl.json';

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    pl: { translation: pl },
  },
  lng: 'en',
  fallbackLng: 'en',
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
