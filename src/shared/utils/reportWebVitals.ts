/** Jaskier Design System */
import type { Metric } from 'web-vitals';

function sendToAnalytics(metric: Metric) {
  // Log to console in development
  if (import.meta.env.DEV) {
    console.log(`[WebVitals] ${metric.name}: ${Math.round(metric.value)}ms (${metric.rating})`);
  }

  // In production, could send to analytics endpoint
  // navigator.sendBeacon('/api/vitals', JSON.stringify(metric));
}

export function reportWebVitals() {
  import('web-vitals').then(({ onCLS, onLCP, onFCP, onTTFB, onINP }) => {
    onCLS(sendToAnalytics);
    onLCP(sendToAnalytics);
    onFCP(sendToAnalytics);
    onTTFB(sendToAnalytics);
    onINP(sendToAnalytics);
  });
}
