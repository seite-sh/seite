## Trust Center

The trust center has three layers:

1. **Data files** (`data/trust/`) — structured YAML that drives the templates
2. **Content pages** (`content/trust/`) — markdown prose for each section
3. **Templates** (`templates/trust-index.html`, `templates/trust-item.html`) — layout (rarely edited)

### Managing Certifications

Edit `data/trust/certifications.yaml` to update certification statuses:

```yaml
- name: SOC 2 Type II
  slug: soc2
  status: active         # active | in_progress | planned
  framework: soc2
  description: >         # shown on trust center hub
    Annual audit covering Security and Availability
  issued: 2025-11-15     # date cert was issued
  expires: 2026-11-15    # expiration date
  auditor: "Deloitte"
  scope: "Security, Availability"
  report_url: "mailto:security@example.com"
```

Status values: `active` (green badge), `in_progress` (yellow), `planned` (gray).

To add a new certification:
1. Add entry to `data/trust/certifications.yaml`
2. Create `content/trust/certifications/{slug}.md` with framework details
3. Run `seite build`

### Managing Subprocessors

Edit `data/trust/subprocessors.yaml`:

```yaml
- name: "AWS"
  purpose: "Cloud infrastructure"
  data_types: ["Customer data", "Logs"]
  location: "United States"
  dpa: true
```

Fields: `name` (required), `purpose`, `data_types` (array), `location`, `dpa` (bool).

### Managing FAQs

Edit `data/trust/faq.yaml`:

```yaml
- question: "Do you encrypt data at rest?"
  answer: "Yes. All data encrypted with AES-256."
  category: encryption     # groups FAQs in the UI
```

Categories: `encryption`, `access`, `data-residency`, `incident-response`, `compliance`, `general`.

### Trust Center Content Pages

Each section is a markdown file in `content/trust/`:

| File | URL | Purpose |
|------|-----|----------|
| `security-overview.md` | `/trust/security-overview` | Main security narrative |
| `vulnerability-disclosure.md` | `/trust/vulnerability-disclosure` | Responsible disclosure |
| `data-processing.md` | `/trust/data-processing` | DPA / data processing terms |
| `certifications/soc2.md` | `/trust/certifications/soc2` | Framework detail page |

Use `weight:` in frontmatter to control section ordering (lower = first).
Use `extra.type:` to categorize: `overview`, `certification`, `policy`, `changelog`.

### Common Trust Center Tasks

```bash
seite new trust "PCI DSS"                    # Add a new certification page
seite new trust "Q1 2026 Security Update"    # Add a changelog entry
seite new trust "Security Overview" --lang es # Create a translation
seite build                                   # Rebuild after editing data files
```

### Multi-language Trust Center

Data files (`data/trust/*.yaml`) are language-neutral. Content pages get translated via the standard i18n system:

```
content/trust/security-overview.md       # English → /trust/security-overview
content/trust/security-overview.es.md    # Spanish → /es/trust/security-overview
```

The trust center index at `/trust/` is rendered per-language automatically.

### MCP Integration

`seite://trust` returns the full trust center state (certifications, subprocessors, FAQs, content items).
Use `seite_search` with `collection: "trust"` to find trust center content.
Use `seite_create_content` with `collection: "trust"` and `extra: {"type": "certification", "framework": "soc2"}` to create trust center pages.
