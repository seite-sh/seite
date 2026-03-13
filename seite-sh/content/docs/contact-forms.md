---
title: Contact Forms for Static Sites
description: "Add contact forms to your seite static site with five built-in providers. Formspree, Web3Forms, Netlify Forms, HubSpot, and Typeform. No backend needed."
weight: 15
---

Static site generators usually punt on contact forms. You end up wiring together third-party scripts yourself. seite handles this natively with a single [shortcode](/docs/shortcodes) that works across five providers. Pick one, add one line of config, drop the shortcode into any page, and you have a working form that matches your theme automatically.

seite supports contact forms through a built-in shortcode that integrates with popular form service providers. No backend code or JavaScript frameworks required for HTML POST providers.

## Quick Start

1. Configure a provider:

```bash
seite contact setup
```

2. Add the shortcode to any page:

```markdown
{{< contact_form() >}}
```

3. Build and [deploy](/docs/deployment):

```bash
seite build && seite deploy
```

## Configuration

Add a `[contact]` section to your `seite.toml` (see the full [configuration reference](/docs/configuration) for all options):

```toml
[contact]
provider = "formspree"
endpoint = "xpznqkdl"
```

### Optional fields

```toml
[contact]
provider = "formspree"
endpoint = "xpznqkdl"
redirect = "/thank-you"      # Custom redirect after submission
subject = "New inquiry"       # Email subject prefix
region = "na1"                # HubSpot only (na1 or eu1)
```

## Supported Providers

### Formspree (HTML POST)

The most popular form service for static sites. Create a form at [formspree.io](https://formspree.io), copy the form ID.

```toml
[contact]
provider = "formspree"
endpoint = "xpznqkdl"
```

Free tier: 50 submissions/month.

### Web3Forms (HTML POST)

Serverless form API with a public-safe access key. Sign up at [web3forms.com](https://web3forms.com) and enter your email to receive an access key.

```toml
[contact]
provider = "web3forms"
endpoint = "YOUR_ACCESS_KEY"
```

Free tier: 250 submissions/month.

### Netlify Forms (HTML POST)

Zero-config when deploying to Netlify. Netlify automatically detects forms with the `data-netlify` attribute at deploy time.

```toml
[contact]
provider = "netlify"
endpoint = "contact"
```

Free tier: 100 submissions/month.

### HubSpot (JS embed)

Embeds a HubSpot form using their JavaScript SDK. Requires a HubSpot account (free CRM available).

```toml
[contact]
provider = "hubspot"
endpoint = "12345678/abcd-1234-efgh-5678"
region = "na1"
```

The endpoint format is `{portalId}/{formGuid}`. Find these in your HubSpot form settings. Set `region = "eu1"` if your account is in the EU data center.

### Typeform (JS embed)

Embeds a Typeform using their embed SDK. Create a form at [typeform.com](https://typeform.com) and copy the form ID.

```toml
[contact]
provider = "typeform"
endpoint = "abc123XY"
```

Free tier: 10 submissions/month.

## Which Provider Should I Choose?

| Provider | Type | Free Tier | Best For |
|----------|------|-----------|----------|
| **Formspree** | HTML POST | 50/month | Simple contact forms, quick setup |
| **Web3Forms** | HTML POST | 250/month | Higher volume, no account required |
| **Netlify** | HTML POST | 100/month | Already deploying to Netlify |
| **HubSpot** | JS embed | Free CRM | Lead tracking and CRM integration |
| **Typeform** | JS embed | 10/month | Interactive, conversational forms |

HTML POST providers render a plain `<form>` tag with no JavaScript: they are faster, more accessible, and work with JS disabled. JS embed providers load an external script and give you a richer form builder UI in exchange.

If you are just starting out and want the least friction, go with Formspree. If you are already [deploying to Netlify](/docs/deployment), use Netlify Forms: zero extra config.

## Shortcode Usage

### Basic

```markdown
{{< contact_form() >}}
```

### With label overrides

```markdown
{{< contact_form(name_label="Full Name", email_label="Work Email", message_label="How can we help?", submit_label="Send") >}}
```

### With per-instance overrides

```markdown
{{< contact_form(subject="Sales Inquiry", redirect="/thank-you") >}}
```

### Typeform height

```markdown
{{< contact_form(height="600px") >}}
```

## CLI Commands

```bash
seite contact setup                                           # Interactive setup
seite contact setup --provider formspree --endpoint xpznqkdl  # Non-interactive
seite contact status                                          # Show current config
seite contact remove                                          # Remove config
```

## Theme Support

All 6 bundled themes include styled contact form CSS. The form automatically matches your theme's design language (colors, borders, border-radius, fonts).

## Spam Protection

HTML POST providers include a honeypot field (hidden from users, caught by bots):

- **Formspree**: `_gotcha` field
- **Web3Forms**: `botcheck` checkbox
- **Netlify**: `bot-field` with `data-netlify-honeypot`

JS embed providers (HubSpot, Typeform) handle spam protection in their own dashboards.

## i18n

Form labels use the `{{ t }}` translation system. Override labels per language in `data/i18n/{lang}.yaml`:

```yaml
contact_name: "Nombre"
contact_email: "Correo"
contact_message: "Mensaje"
contact_submit: "Enviar"
```

Or use shortcode args for one-off overrides.

## Next Steps

- [Configuration](/docs/configuration): full `[contact]` section reference and all `seite.toml` options
- [Getting Started](/docs/getting-started): set up your first seite site from scratch
- [Deployment](/docs/deployment): deploy your site with the contact form live
- [Shortcodes](/docs/shortcodes): other built-in content components like callouts, figures, and video embeds
