# Trust Center / Compliance Hub — Implementation Plan

## Overview

Add a `trust` collection preset with full init scaffolding, data-driven templates, MCP integration, CLAUDE.md onboarding, and multi-language support. The trust center is a first-class feature that generates a compliance hub with certifications, subprocessor lists, security FAQs, and policy pages — all managed through data files and markdown content.

---

## Phase 1: Collection Preset & Config

### 1.1 Add `trust` collection preset

**File: `src/config/mod.rs`**

Add `preset_trust()`:
```rust
pub fn preset_trust() -> Self {
    Self {
        name: "trust".into(),
        label: "Trust Center".into(),
        directory: "trust".into(),
        has_date: false,
        has_rss: false,
        listed: true,
        url_prefix: "/trust".into(),
        nested: true,        // for certifications/ subdirectory
        default_template: "trust-item.html".into(),
        paginate: None,
    }
}
```

Update `from_preset()` to include `"trust"` case.

Update `find_collection()` singular normalization — `"trust"` is already singular, no change needed.

### 1.2 Add trust-specific config types

**File: `src/config/mod.rs`**

Add a `TrustSection` to hold trust-center-specific config that lives in `page.toml`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrustSection {
    /// Company name displayed on the trust center (defaults to site.title)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Active compliance frameworks (e.g., ["soc2", "iso27001"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
}
```

Add `trust: Option<TrustSection>` to `SiteConfig` with `skip_serializing_if = "Option::is_none"`.

---

## Phase 2: Init Flow — Interactive Scaffolding

### 2.1 Add CLI args to `InitArgs`

**File: `src/cli/init.rs`**

```rust
/// Company name for trust center (defaults to --title)
#[arg(long)]
pub trust_company: Option<String>,

/// Compliance frameworks (comma-separated: soc2,iso27001,gdpr,hipaa,pci-dss,ccpa,soc3)
#[arg(long)]
pub trust_frameworks: Option<String>,

/// Trust center sections (comma-separated: overview,certifications,subprocessors,faq,disclosure,dpa,changelog)
#[arg(long)]
pub trust_sections: Option<String>,
```

### 2.2 Interactive trust center setup

When `trust` is in the selected collections, trigger `scaffold_trust_center()`:

1. **Company name** — `dialoguer::Input` defaulting to site title
2. **Framework selection** — `dialoguer::MultiSelect` with options:
   - SOC 2 Type II
   - ISO 27001
   - GDPR
   - HIPAA
   - PCI DSS
   - CCPA / CPRA
   - SOC 3
   Default: SOC 2 Type II selected
3. **Section selection** — `dialoguer::MultiSelect` with options:
   - Security Overview
   - Certifications (auto-selected if any frameworks chosen)
   - Subprocessor List
   - FAQ / Security Questionnaire
   - Vulnerability Disclosure
   - Data Processing Agreement
   - Security Changelog / Updates
   Default: overview, certifications, subprocessors, faq, disclosure
4. **Per-framework status** — for each selected framework, `dialoguer::Select`:
   - Active (certified)
   - In Progress (pursuing)
   - Planned (on roadmap)

### 2.3 Scaffolded files

The `scaffold_trust_center()` function creates:

**Data files (`data/trust/`):**
- `certifications.yaml` — one entry per selected framework with status, placeholder dates
- `subprocessors.yaml` — template with 2 example entries + commented template
- `faq.yaml` — pre-filled Q&A entries relevant to selected frameworks (8-14 questions)
- `sections.yaml` — ordered list of enabled sections for template rendering

**Content files (`content/trust/`):**
- `security-overview.md` — if overview section selected (weight: 1)
- `vulnerability-disclosure.md` — if disclosure section selected (weight: 5)
- `data-processing.md` — if DPA section selected (weight: 6)
- `certifications/{framework-slug}.md` — one per selected framework (weight: 2)

**Templates (in `templates/`):**
- `trust-index.html` — already provided as embedded default (no file written unless user customizes)
- `trust-item.html` — already provided as embedded default

**Multi-language:** If `[languages.*]` is configured at init time, scaffold translated content files too (`.es.md`, `.fr.md`, etc.) with translated frontmatter titles. The data files are language-neutral.

### 2.4 Write `[trust]` to page.toml

Add the `TrustSection` with company name and frameworks list to the generated config.

---

## Phase 3: Templates

### 3.1 `trust-item.html` — individual trust page template

**File: `src/templates/mod.rs`**

Add `DEFAULT_TRUST_ITEM` constant and wire it into `get_default_template()`.

Simple template extending base.html with trust-specific layout:
- Title + description
- Content body
- If `page.extra.type == "certification"`, show structured cert data from `data.trust.certifications` matching `page.extra.framework`
- Back link to trust center index

### 3.2 `trust-index.html` — hub landing page template

**File: `src/templates/mod.rs`**

Add `DEFAULT_TRUST_INDEX` constant. This is the main trust center page rendered at `/trust/`.

Data-driven sections (all conditional on data existence):
1. **Hero** — company name + headline
2. **Certification cards** — grid from `data.trust.certifications`, status badges
3. **Content sections** — trust collection items ordered by weight
4. **Subprocessor table** — rendered from `data.trust.subprocessors`
5. **FAQ accordion** — rendered from `data.trust.faq`, grouped by category
6. **CTA footer** — "Request our report" link

Register as the index template for the trust collection in the build pipeline. This requires a small change to the build pipeline to detect when a collection has a custom index template (trust-index.html) and render it at the collection's URL prefix.

### 3.3 Theme CSS for trust center

**All 6 theme files: `src/themes/{default,minimal,dark,docs,brutalist,bento}.tera`**

Add CSS rules for:
- `.trust-hero` — hero section styling
- `.cert-grid` — certification card grid
- `.cert-card` — individual cert card
- `.cert-status` — status badge (`.active` green, `.in-progress` yellow, `.planned` gray)
- `.subprocessor-table` — responsive data table
- `.faq-section` — FAQ container
- `.faq-item` — individual Q&A with details/summary accordion
- `.trust-cta` — call-to-action section

Each theme styles these consistently with its own design language.

---

## Phase 4: MCP Integration

### 4.1 `page://trust` resource

**File: `src/mcp/resources.rs`**

Add a new resource `page://trust` that exposes the trust center state:
- Returns combined view of:
  - Trust config from `page.toml` (company, frameworks)
  - Certification data from `data/trust/certifications.yaml`
  - Subprocessor count from `data/trust/subprocessors.yaml`
  - FAQ count from `data/trust/faq.yaml`
  - Content items in the trust collection
- Only listed when the `trust` collection exists in config

This gives the AI agent a single resource to understand the full trust center state.

### 4.2 Update `page_create_content` tool

**File: `src/mcp/tools.rs`**

The existing `page_create_content` tool already works with any collection. Add `extra` parameter support so the agent can set `extra.type`, `extra.framework`, etc. when creating trust content:

```json
{
  "name": "page_create_content",
  "inputSchema": {
    "properties": {
      "extra": {
        "type": "object",
        "description": "Extra frontmatter fields (key-value pairs)"
      }
    }
  }
}
```

### 4.3 Update `page_search` tool

Already searches all collections including trust — no changes needed. The agent can use `page_search(query: "soc2", collection: "trust")` to find trust center content.

---

## Phase 5: CLAUDE.md Integration (Critical Onboarding Surface)

### 5.1 Trust center section in `generate_claude_md()`

**File: `src/cli/init.rs`**

When the trust collection is present, add a comprehensive trust center section to the scaffolded CLAUDE.md. This is the primary onboarding surface — the AI agent reads this to understand how to manage the trust center.

```markdown
## Trust Center

This site includes a compliance trust center at `/trust/`. The trust center is
data-driven — most changes are made by editing YAML files in `data/trust/`,
not by editing templates.

### Company: {company_name}
### Active Frameworks: {framework_list}

### How the Trust Center Works

The trust center has three layers:
1. **Data files** (`data/trust/`) — structured data that drives the templates
2. **Content pages** (`content/trust/`) — markdown prose for each section
3. **Templates** (`templates/trust-index.html`, `templates/trust-item.html`) — layout (rarely edited)

### Managing Certifications

Edit `data/trust/certifications.yaml` to update certification statuses:

```yaml
- name: SOC 2 Type II
  slug: soc2
  status: active         # active | in_progress | planned
  issued: 2025-11-15     # date cert was issued
  expires: 2026-11-15    # expiration date
  auditor: "Deloitte"
  scope: "Security, Availability"
  report_url: "mailto:security@example.com"  # or gated URL
```

Status values and their display:
- `active` — green badge, shows issued/expires dates
- `in_progress` — yellow badge, shows target date if set
- `planned` — gray badge, shown on roadmap

To add a new certification:
1. Add entry to `data/trust/certifications.yaml`
2. Create `content/trust/certifications/{slug}.md` with details
3. Run `seite build`

To create a translated certification page:
```bash
seite new trust "SOC 2 Type II" --lang es
# Then edit content/trust/certifications/soc-2-type-ii.es.md
```

### Managing Subprocessors

Edit `data/trust/subprocessors.yaml`:

```yaml
- name: "AWS"
  purpose: "Cloud infrastructure"
  data_types: ["Customer data", "Logs"]
  location: "United States"
  dpa: true

- name: "Stripe"
  purpose: "Payment processing"
  data_types: ["Billing data"]
  location: "United States"
  dpa: true
```

Fields: `name` (required), `purpose`, `data_types` (array), `location`, `dpa` (bool),
`certifications` (array, optional).

### Managing FAQs

Edit `data/trust/faq.yaml`:

```yaml
- question: "Do you encrypt data at rest?"
  answer: "Yes. All data encrypted with AES-256."
  category: encryption         # groups FAQs in the UI
  frameworks: [soc2, iso27001] # which frameworks this answers
```

Categories: `encryption`, `access`, `data-residency`, `incident-response`,
`compliance`, `infrastructure`, `general`

### Trust Center Content Pages

Each section is a markdown file in `content/trust/`:

| File | URL | Purpose |
|------|-----|---------|
| `security-overview.md` | `/trust/security-overview` | Main security narrative |
| `vulnerability-disclosure.md` | `/trust/vulnerability-disclosure` | Responsible disclosure program |
| `data-processing.md` | `/trust/data-processing` | DPA / data processing terms |
| `certifications/soc2.md` | `/trust/certifications/soc2` | SOC 2 detail page |
| `certifications/iso27001.md` | `/trust/certifications/iso27001` | ISO 27001 detail page |

Use `weight:` in frontmatter to control section ordering (lower = first).
Use `extra.type:` to categorize: `overview`, `certification`, `policy`, `changelog`.

### Trust Center Templates

The trust center index (`/trust/`) renders data-driven sections:
- Certification grid with status badges (from `data.trust.certifications`)
- Content sections from trust collection items (ordered by `weight`)
- Subprocessor table (from `data.trust.subprocessors`)
- FAQ accordion (from `data.trust.faq`)

All sections are conditional — if a data file is empty/missing, the section hides.

### Common Trust Center Tasks

```bash
# Add a new certification
seite new trust "PCI DSS" --tags certification

# Add a security update / changelog entry
seite new trust "Q1 2026 Security Update" --tags changelog

# Create a translated trust page
seite new trust "Security Overview" --lang es

# Rebuild after editing data files
seite build
```

### Multi-language Trust Center

Data files (`data/trust/*.yaml`) are language-neutral — dates, statuses, and
vendor names don't change per language.

Content pages get translated via the standard i18n system:
- `content/trust/security-overview.md` → English at `/trust/security-overview`
- `content/trust/security-overview.es.md` → Spanish at `/es/trust/security-overview`

The trust center index is rendered per-language automatically.
```

### 5.2 MCP awareness in CLAUDE.md

Add to the MCP Server section:

```markdown
**Trust center resources:** `page://trust` returns the full trust center state
(certifications, subprocessors, FAQs, content items). Use `page_search` with
`collection: "trust"` to find trust center content. Use `page_create_content`
with `collection: "trust"` and `extra: {"type": "certification", "framework": "soc2"}`
to create trust center pages.
```

---

## Phase 6: Embedded Documentation

### 6.1 Add trust center doc page

**File: `src/docs/trust-center.md`**

Create embedded doc covering:
- What the trust center is
- How to add it to a new or existing site
- Data file formats (certifications, subprocessors, FAQ)
- Content page conventions
- Template customization
- Multi-language support
- Common workflows

### 6.2 Register in `src/docs.rs`

Add `trust_center()` function returning a `DocPage` with weight 7 (after deployment, before agent).

### 6.3 Mirror to `seite-sh/content/docs/trust-center.md`

Copy the same content for the deployed documentation site.

---

## Phase 7: Build Pipeline Integration

### 7.1 Trust center index rendering

**File: `src/build/mod.rs`**

The trust collection already renders individual pages through the standard collection pipeline. For the trust center index page at `/trust/`:

- Detect if a collection has `name == "trust"`
- Render `trust-index.html` template at the collection's URL prefix with:
  - `collections` context (trust items)
  - `data.trust.*` (certifications, subprocessors, FAQ data)
  - Standard `site`, `lang`, `translations` context

This follows the same pattern as the existing paginated collection index rendering.

---

## Phase 8: Tests

### 8.1 Unit tests

- `test_trust_preset_config` — verify preset values
- `test_trust_section_serialization` — TrustSection round-trips through TOML
- `test_trust_data_templates` — verify scaffolded YAML is valid

### 8.2 Integration tests (`tests/integration.rs`)

- `test_init_with_trust_collection` — `seite init mysite --collections trust,pages --trust-company "Acme" --trust-frameworks soc2,iso27001 --trust-sections overview,certifications,faq` creates all expected files
- `test_build_trust_center` — build succeeds with scaffolded trust center
- `test_trust_center_index_rendered` — `/trust/` index page exists in output
- `test_trust_certification_pages` — individual cert pages render
- `test_trust_multilingual` — trust center works with i18n
- `test_trust_mcp_resource` — `page://trust` resource returns valid data
- `test_trust_search` — `page_search(query: "soc2", collection: "trust")` finds content

### 8.3 Update existing tests

- `test_all_returns_all_docs` assertion count: 12 → 13 (or >=13)
- `test_list_returns_all_tools` if any new tools added
- Any deploy test fixtures that construct `SiteConfig`

---

## Phase 9: CLAUDE.md & Docs Updates

### 9.1 Update project CLAUDE.md

- Add `trust` to collection presets table
- Add trust center to module map
- Add `TrustSection` to config example
- Update init CLI args
- Add trust center to the "Adding a User-Facing Feature" checklist

### 9.2 Update embedded docs

- `src/docs/configuration.md` — add `[trust]` section
- `src/docs/collections.md` — add trust preset
- `src/docs/cli-reference.md` — add `--trust-*` flags

---

## Supported Compliance Frameworks (presets)

| Slug | Display Name | Category |
|------|-------------|----------|
| `soc2` | SOC 2 Type II | Audit |
| `iso27001` | ISO 27001 | Certification |
| `gdpr` | GDPR | Regulation |
| `hipaa` | HIPAA | Regulation |
| `pci-dss` | PCI DSS | Certification |
| `ccpa` | CCPA / CPRA | Regulation |
| `soc3` | SOC 3 | Audit |

Each framework gets pre-written FAQ questions and a content page template.

---

## File Inventory (new/modified)

**New files:**
- `src/docs/trust-center.md`
- `seite-sh/content/docs/trust-center.md`

**Modified files:**
- `src/config/mod.rs` — `TrustSection`, `preset_trust()`, `from_preset()`
- `src/cli/init.rs` — `InitArgs` trust flags, `scaffold_trust_center()`, `generate_claude_md()` trust section
- `src/templates/mod.rs` — `DEFAULT_TRUST_INDEX`, `DEFAULT_TRUST_ITEM`, `get_default_template()`
- `src/mcp/resources.rs` — `page://trust` resource
- `src/mcp/tools.rs` — `extra` field support in `page_create_content`
- `src/docs.rs` — `trust_center()` entry
- `src/build/mod.rs` — trust center index rendering
- `src/themes/{default,minimal,dark,docs,brutalist,bento}.tera` — trust center CSS
- `tests/integration.rs` — trust center tests
- `CLAUDE.md` — documentation updates

---

## Implementation Order

1. Config (preset + TrustSection) — foundation everything else depends on
2. Templates (trust-index.html + trust-item.html) — needed before build works
3. Build pipeline (trust index rendering) — needed before init can be tested end-to-end
4. Init flow (interactive prompts + scaffolding) — the main feature
5. CLAUDE.md integration — critical onboarding surface
6. MCP integration (resource + tool update) — agent support
7. Embedded docs — discoverable via MCP
8. Theme CSS — visual polish
9. Tests — validate everything
10. Project CLAUDE.md + docs updates — housekeeping
