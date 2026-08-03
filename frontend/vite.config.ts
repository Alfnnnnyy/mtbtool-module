import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import path from 'path';

export default defineConfig({
  base: './',
  plugins: [svelte()],
  resolve: {
    alias: {
      '$lib': path.resolve(__dirname, './src/lib')
    }
  },
  build: {
    outDir: '../webroot',
    emptyOutDir: true,
    target: 'es2020',
    rollupOptions: {
      external: ['kernelsu']
    }
  },
  test: {
    environment: 'node',
    globals: true
  }
});
