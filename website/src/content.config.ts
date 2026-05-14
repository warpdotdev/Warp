import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const docs = defineCollection({
  loader: glob({ pattern: '**/*.{md,mdx}', base: './src/content/docs' }),
  schema: z.object({
    title: z.string(),
    title_zh: z.string(),
    title_ja: z.string(),
    lede: z.string(),
    lede_zh: z.string(),
    lede_ja: z.string(),
    order: z.number().default(0),
    section: z.string().default('Get Started'),
    section_zh: z.string().default('快速开始'),
    section_ja: z.string().default('はじめに'),
  }),
});

export const collections = { docs };
