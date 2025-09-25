import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const config: Config = {
  stylesheets: [
    {
      href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=IBM+Plex+Mono:wght@400;500;600;700&display=swap",
      type: "text/css",
    },
  ],

  title: "VES - Vector Enhanced Search",
  tagline: "Semantic log search at scale.",
  favicon: "img/favicon.ico", // TODO: swap for H3IMD3LL Labs logo

  future: {
    v4: true,
  },

  organizationName: "H3IMD3LL Labs, Inc.",
  projectName: "VES",

  url: "https://ves.heimdelllabs.cloud",
  baseUrl: "/",

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "warn",

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      {
        docs: {
          sidebarPath: "./sidebars.ts",
          editUrl: "https://github.com/H3IMD3LL-Labs-Inc/VES",
        },
        blog: {
          showReadingTime: true,
          feedOptions: {
            type: ["rss", "atom"],
            xslt: true,
          },
          editUrl: "https://github.com/H3IMD3LL-Labs-Inc/VES",
          onInlineTags: "warn",
          onInlineAuthors: "warn",
          onUntruncatedBlogPosts: "warn",
        },
        theme: {
          customCss: "./src/css/custom.css", // override Infima with Inter + brand colors
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    image: "img/social-card.png", // TODO: replace with VES social card
    colorMode: {
      defaultMode: "light",
      disableSwitch: false, // allow light/dark toggle like Chroma
      respectPrefersColorScheme: true,
    },
    navbar: {
      style: "primary", // makes it sleek like Chroma's top nav
      logo: {
        alt: "VES Logo",
        src: "img/logo.svg", // TODO: replace with H3IMD3LL Labs logo
      },
      items: [
        { to: "docs/overview/introduction", label: "Docs", position: "left" },
        { to: "/blog", label: "Updates", position: "left" },
        { to: "pages/pricing", label: "Pricing", position: "left" },
        {
          href: "https://github.com/H3IMD3LL-Labs-Inc/VES",
          label: "GitHub",
          position: "right",
        },
        {
          to: "docs/overview/getting-started",
          label: "Get started",
          position: "right",
          className: "navbar__link--cta", // styled in custom.css as a button
        },
      ],
    },
    footer: {
      style: "light",
      links: [
        {
          title: "Product",
          items: [
            { label: "Docs", to: "/docs/overview/introduction" },
            { label: "Pricing", to: "pages/pricing" },
            { label: "Changelog", to: "pages/changelog" },
          ],
        },
        {
          title: "Community",
          items: [
            {
              label: "Discord",
              href: "https://discordapp.com/invite/heimdelllabs",
            },
            { label: "GitHub", href: "https://github.com/H3IMD3LL-Labs-Inc" },
            { label: "~~Twitter~~ X", href: "https://x.com/heimdell_labs" },
          ],
        },
        {
          title: "Company",
          items: [
            { label: "About", to: "/docs/overview/about" },
            { label: "Careers", href: "https://careers.heimdelllabs.cloud" },
          ],
        },
        {
          title: "Legal",
          items: [
            { label: "Privacy", to: "pages/website-privacy" },
            { label: "Terms", to: "pages/website-terms" },
            { label: "Security", to: "pages/security" },
          ],
        },
      ],
      copyright: `© ${new Date().getFullYear()} H3IMD3LL Labs, Inc.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
