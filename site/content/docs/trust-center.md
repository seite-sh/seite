---
title: Trust Center
description: Build a compliance hub with certifications, subprocessors, FAQs, and security policies
slug: trust-center
weight: 7
---

# Trust Center

The trust center is a built-in collection preset that scaffolds a compliance hub for your site. It generates data-driven pages for certifications, subprocessor lists, security FAQs, and policy documents.

## Getting Started

### Add during site creation

```bash
page init mysite --collections posts,pages,trust
```

When `trust` is included, you'll be prompted to select:
- **Company name** — displayed on the trust center
- **Compliance frameworks** — SOC 2, ISO 27001, GDPR, HIPAA, PCI DSS, CCPA, SOC 3
- **Sections** — Security Overview, Certifications, Subprocessors, FAQ, Vulnerability Disclosure, DPA, Changelog
- **Per-framework status** — Active, In Progress, or Planned

### Non-interactive (CI-friendly)

```bash
page init mysite \
  --collections posts,pages,trust \
  --trust-company "Acme Corp" \
  --trust-frameworks soc2,iso27001 \
  --trust-sections overview,certifications,subprocessors,faq,disclosure
```

## Architecture

The trust center has three layers:

1. **Data files** (`data/trust/`) — structured YAML that drives the templates
2. **Content pages** (`content/trust/`) — markdown prose for each section
3. **Templates** — `trust-index.html` (hub page at `/trust/`) and `trust-item.html` (individual pages)

### Scaffolded File Structure

```
data/trust/
├── certifications.yaml    # framework entries with status, dates, auditor
├── subprocessors.yaml     # vendor table with name, purpose, location, DPA
└── faq.yaml               # security Q&A grouped by category

content/trust/
├── security-overview.md           # main security narrative (weight: 1)
├── vulnerability-disclosure.md    # responsible disclosure program (weight: 5)
├── data-processing.md             # DPA / data processing terms (weight: 6)
└── certifications/
    ├── soc2.md                    # per-framework detail pages (weight: 2)
    └── iso27001.md
```

## Data Files

### Certifications (`data/trust/certifications.yaml`)

```yaml
- name: SOC 2 Type II
  slug: soc2
  status: active           # active | in_progress | planned
  framework: soc2
  description: >
    Annual audit covering Security and Availability trust service criteria
  issued: 2025-11-15       # date cert was issued
  expires: 2026-11-15      # expiration date
  auditor: "Deloitte"
  scope: "Security, Availability"
  report_url: "mailto:security@example.com"
```

Status values control the badge display:
- `active` — green badge, shows issued/expires dates
- `in_progress` — yellow badge, shows target date
- `planned` — gray badge, shown on roadmap

### Subprocessors (`data/trust/subprocessors.yaml`)

```yaml
- name: "AWS"
  purpose: "Cloud infrastructure and hosting"
  data_types: ["Customer data", "Application logs"]
  location: "United States"
  dpa: true
```

Fields: `name` (required), `purpose`, `data_types` (array), `location`, `dpa` (boolean).

### FAQ (`data/trust/faq.yaml`)

```yaml
- question: "Do you encrypt data at rest?"
  answer: "Yes. All customer data is encrypted at rest using AES-256 encryption."
  category: encryption
```

Categories group FAQs in the UI: `encryption`, `access`, `data-residency`, `incident-response`, `compliance`, `general`.

## Content Pages

Trust center content files use the standard markdown + YAML frontmatter format. Key frontmatter fields:

```yaml
---
title: "Security Overview"
description: "How Acme Corp protects your data"
weight: 1                    # controls section ordering (lower = first)
extra:
  type: overview             # overview | certification | policy | changelog
  framework: soc2            # links to data/trust/certifications.yaml entry
---
```

The `extra.type` field categorizes the page:
- `overview` — main security narrative
- `certification` — framework detail page (paired with `extra.framework`)
- `policy` — vulnerability disclosure, DPA, etc.
- `changelog` — security updates and changes

## Trust Center Index

The hub page at `/trust/` is rendered using `trust-index.html` and displays:

1. **Hero section** — company name and headline
2. **Certification grid** — cards from `data.trust.certifications` with status badges
3. **Content sections** — trust collection items ordered by weight
4. **Subprocessor table** — from `data.trust.subprocessors`
5. **FAQ accordion** — from `data.trust.faq`

All sections are conditional — if a data file is empty or missing, the section doesn't render.

## Configuration

The `[trust]` section in `page.toml` stores trust center metadata:

```toml
[trust]
company = "Acme Corp"
frameworks = ["soc2", "iso27001"]
```

- `company` — displayed on the trust center (defaults to `site.title`)
- `frameworks` — list of active framework slugs

## Common Tasks

```bash
# Add a new certification
page new trust "PCI DSS"

# Add a security update
page new trust "Q1 2026 Security Update" --tags changelog

# Create a translation
page new trust "Security Overview" --lang es

# Rebuild after editing data files
page build
```

## Multi-language Support

Data files (`data/trust/*.yaml`) are language-neutral — structured data like dates, statuses, and vendor names don't change per language.

Content pages get translated using the standard i18n filename convention:

```
content/trust/security-overview.md       # English → /trust/security-overview
content/trust/security-overview.es.md    # Spanish → /es/trust/security-overview
```

The trust center index is rendered per-language automatically.

## Template Customization

Override `templates/trust-index.html` or `templates/trust-item.html` to customize the layout. Template variables available:

- `data.trust.certifications` — array of certification objects
- `data.trust.subprocessors` — array of vendor objects
- `data.trust.faq` — array of Q&A objects
- `collections` — trust collection items (on index page)
- `page.extra.type` — content type (on item pages)
- `page.extra.framework` — framework slug for certification pages

## MCP Integration

The `page://trust` resource returns the full trust center state as JSON, including certifications, subprocessor count, FAQ count, and content items. Use `page_search` with `collection: "trust"` to find trust center content.
