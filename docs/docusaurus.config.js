// @ts-check
// Note: type annotations allow type checking and IDEs autocompletion

const lightCodeTheme = require('prism-react-renderer/themes/duotoneLight');
const darkCodeTheme = require('prism-react-renderer/themes/duotoneDark');

/** @type {import('@docusaurus/types').Config} */
const config = {
	title: 'Composable Finance',
	tagline: 'The interoperable infrastructure for Modular DeFi',
	url: 'https://composable.finance',
	baseUrl: '/',
	onBrokenLinks: 'warn',
	onBrokenMarkdownLinks: 'warn',
	favicon: 'img/favicon.ico',

	// GitHub pages deployment config.
	// If you aren't using GitHub pages, you don't need these.
	organizationName: 'Composable Finance', // Usually your GitHub org/user name.
	projectName: 'composable', // Usually your repo name.

	// Even if you don't use internalization, you can use this field to set useful
	// metadata like html lang. For example, if your site is Chinese, you may want
	// to replace "en" with "zh-Hans".
	i18n: {
		defaultLocale: 'en',
		locales: ['en'],
	},

	presets: [
		[
			'classic',
			/** @type {import('@docusaurus/preset-classic').Options} */
			({
				docs: {
					breadcrumbs: true,
					sidebarPath: require.resolve('./sidebars.js'),
					routeBasePath: '/',
					// Please change this to your repo.
					// Remove this to remove the "edit this page" links.
					editUrl: 'https://github.com/ComposableFi/composable/tree/main/docs/',
				},
				blog: false,
				theme: {
					customCss: require.resolve('./src/css/custom.css'),
				},
			}),
		],
	],

	themeConfig:
		/** @type {import('@docusaurus/preset-classic').ThemeConfig} */
		({
	    algolia: {
	      // The application ID provided by Algolia
	      appId: '1GMXVIRCBW',
	      // Public API key: it is safe to commit it
	      apiKey: 'de939a9de56cd5e30ef4a25b9f61a641',
	      indexName: 'composable',
	    },
			navbar: {
				title: 'Composable Finance',
				logo: {
					alt: 'Composable Finance Logo',
					src: 'img/logo.svg',
				},
				items: [
					{
						to: '/networks/picasso-parachain-overview',
						position: 'left',
						label: 'Networks',
					  },
					{
						to: '/products/centauri-overview',
						position: 'left',
						label: 'Centauri',
					  },
					{
						to: '/products/xcvm',
						position: 'left',
						label: 'Composable VM',
					  },
					{
						to: '/nix/install',
						position: 'left',
						label: 'Develop',
					  },
					{
						to: '/ecosystem/build-on-composable-ecosystem-development',
						position: 'left',
						label: 'Ecosystem',
					  },	
					{
						to: '/accounts-wallets',
						position: 'left',
						label: 'User Guides',
					  },				
					{
						href: 'https://github.com/ComposableFi/composable',
						label: 'GitHub',
						position: 'right',
					},
					{
						href: 'https://composablefi.medium.com/',
						label: 'Medium',
						position: 'right',
					},
				],
			},
			footer: {
				style: 'dark',
				links: [
					{
						title: 'Community',
						items: [
							{
								label: 'Composable Twitter',
								href: 'https://twitter.com/composablefin',
							},
							{
								label: 'Picasso Twitter',
								href: 'https://twitter.com/Picasso_Network',
							},
							{
								label: 'Telegram',
								href: 'https://t.me/https://t.me/composablefinance',
							},
							{
								label: 'Discord',
								href: 'https://discord.gg/composable',
							},
							{
								label: 'LinkedIn',
								href: 'https://www.linkedin.com/company/composable-finance/',
							},
						],
					},
					{
						title: 'More',
						items: [
							{
								label: 'GitHub',
								href: 'https://github.com/ComposableFi/composable',
							},
							{
								label: 'Composable Medium',
								href: 'https://composablefi.medium.com',
							},
							{
								label: 'Picasso Medium',
								href: 'https://medium.com/@picasso_network',
							},
							{
								label: 'Press Kit',
								href: 'https://docs.composable.finance/ecosystem/press-kit',
							},
						],
					},
				],
				copyright: `Copyright © ${new Date().getFullYear()} Composable Finance, Ltd.`,
			},
			prism: {
        additionalLanguages: ['rust', 'haskell', 'nix'],
				theme: lightCodeTheme,
				darkTheme: darkCodeTheme,
			},
		}),
	plugins: ['docusaurus-plugin-sass', 'my-loaders'],
};

module.exports = config;