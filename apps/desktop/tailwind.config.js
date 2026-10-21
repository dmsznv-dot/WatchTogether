/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        player: {
          bg: '#0B0D11',
          surface: '#14171F',
          card: '#1E222D',
          border: '#2A2F3D',
          hover: '#262B3A',
          accent: '#6366F1', // Indigo
          accentHover: '#4F46E5',
        },
      },
      fontFamily: {
        mono: ['ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
};
