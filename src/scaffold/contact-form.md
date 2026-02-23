## Contact Forms

The `{{< contact_form() >}}` shortcode renders a styled contact form using the configured provider. All 6 bundled themes include contact form CSS.

### Configuration

```toml
[contact]
provider = "formspree"   # formspree, web3forms, netlify, hubspot, typeform
endpoint = "xpznqkdl"    # provider-specific ID
# redirect = "/thank-you"  # optional redirect after submission
# subject = "New inquiry"  # optional email subject prefix
# region = "na1"           # HubSpot only (na1 or eu1)
```

### Providers

| Provider | Endpoint format | Type | Free tier |
|----------|----------------|------|-----------|
| Formspree | Form ID (e.g., `xpznqkdl`) | HTML POST | 50/month |
| Web3Forms | Access key | HTML POST | 250/month |
| Netlify Forms | Form name (e.g., `contact`) | HTML POST | 100/month |
| HubSpot | `{portalId}/{formGuid}` | JS embed | Free CRM |
| Typeform | Form ID (e.g., `abc123XY`) | JS embed | 10/month |

### Shortcode Usage

```markdown
{{< contact_form() >}}

# With label overrides:
{{< contact_form(name_label="Your Name", submit_label="Send") >}}

# With per-instance subject:
{{< contact_form(subject="Sales Inquiry") >}}
```

### CLI Commands

- `seite contact setup` — interactive provider configuration
- `seite contact setup --provider formspree --endpoint xpznqkdl` — non-interactive
- `seite contact status` — show current config
- `seite contact remove` — remove contact form config
