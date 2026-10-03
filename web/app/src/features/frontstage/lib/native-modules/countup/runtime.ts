import CountUpModule, { useCountUp } from 'react-countup';

// Vite's CommonJS interop can expose the exports object as the default import;
// production and test loaders may already unwrap it. Keep that distinction here.
const CountUp = (
  typeof CountUpModule === 'function'
    ? CountUpModule
    : (CountUpModule as unknown as { default: typeof CountUpModule }).default
);

export { CountUp as default, useCountUp };
