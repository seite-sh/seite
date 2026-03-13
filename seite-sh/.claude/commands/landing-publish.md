# Landing Page Publish Command

Use this command to publish landing pages to the seite site as pages.

## Usage
`/landing-publish [file path] [options]`

**Options:**
- `--noindex`: Set `robots: noindex` in frontmatter (for PPC pages)
- `--draft`: Keep as draft instead of publishing immediately

**Examples:**
- `/landing-publish content/pages/product-hosting-beginners.md`
- `/landing-publish content/pages/free-trial-ppc.md --noindex`
- `/landing-publish content/pages/pricing-comparison.md --draft`

## What This Command Does

1. Validates the landing page file
2. Checks landing page score (must be >=75)
3. Verifies frontmatter format and required fields
4. Runs `seite build` to verify it builds correctly
5. Optionally deploys with `seite deploy`

## Prerequisites

Before publishing, ensure:
1. Landing page score is >=75 (run `/landing-audit` first)
2. No critical issues remain
3. All required frontmatter is present
4. Content has been scrubbed for formatting artifacts

## File Format Requirements

Landing page files must be in `content/pages/` with proper seite frontmatter:

```yaml
---
title: "Benefit-Focused Headline (50-60 chars)"
description: "Compelling meta description with primary keyword (150-160 chars)"
slug: page-slug
tags:
  - landing-page
  - target-keyword
image: /static/og-landing.png
extra:
  page_type: seo
  conversion_goal: trial
  target_keyword: "primary keyword"
---

[Content...]
```

## Publishing Process

### Step 1: Validation

Check file exists in `content/pages/` and contains required frontmatter:
- `title` (required)
- `description` (required)
- `slug` (required)
- `extra.target_keyword` (required for SEO pages)
- `extra.page_type` (seo or ppc)
- `extra.conversion_goal` (trial, demo, or lead)

### Step 2: Score Check

Run landing page scorer on the content:
- If score < 75, abort publishing
- Display current score and critical issues
- Suggest running `/landing-audit` for full analysis

### Step 3: Frontmatter Adjustments

1. Ensure `draft: true` is removed (or set to `false`)
2. If `--noindex` flag is set, add `robots: noindex` to frontmatter
3. Verify `description` is 150-160 characters
4. Verify `title` is 50-60 characters

### Step 4: Build Verification

Run `seite build` to verify the page builds correctly:
- Check for template errors
- Verify the page appears at the expected URL
- Confirm no broken internal links

### Step 5: Deploy (Optional)

Ask user if they want to deploy:
- If yes, run `seite deploy`
- If no, remind them to deploy later with `seite deploy`

## Output

### Successful Publish
```
=== Landing Page Published ===

Status: Ready to deploy
File: content/pages/product-hosting-beginners.md
URL: /product-hosting-beginners
Landing Page Score: [X]/100

Next Steps:
1. Preview with `seite serve`
2. Deploy with `seite deploy` when ready
3. Verify live page loads correctly
```

### Failed Publish
```
=== Publishing Failed ===

Reason: [Error message]

If score too low:
- Current Score: [X]/100
- Required Score: 75/100
- Critical Issues:
  1. [Issue 1]
  2. [Issue 2]

Run `/landing-audit content/pages/[file].md` for full analysis.
```

## Differences from /publish-draft

| Aspect | /publish-draft (Blog) | /landing-publish (Pages) |
|--------|----------------------|--------------------------|
| Content Type | Blog post | Landing page |
| Location | content/posts/ | content/pages/ |
| Score Required | Content score >=70 | Landing page score >=75 |
| noindex Option | No | Yes (for PPC) |
| Output URL | /posts/slug | /slug (root level) |

## Pre-Publish Checklist

Before running this command, verify:

### Content
- [ ] Headline is benefit-focused
- [ ] Value proposition is clear
- [ ] CTAs use action verbs
- [ ] Trust signals present
- [ ] Risk reversal near CTAs
- [ ] FAQ section (for SEO pages)

### Meta
- [ ] Title 50-60 characters
- [ ] Title includes keyword
- [ ] Description 150-160 characters
- [ ] Description includes CTA
- [ ] URL slug is clean and short

### Technical
- [ ] Content scrubbed for formatting artifacts
- [ ] Landing page score >=75
- [ ] No critical issues
- [ ] Proper markdown formatting
- [ ] File is in content/pages/ directory

## Post-Publish Tasks

After deploying:

1. **Verify Live Page**
   - Check formatting displays correctly
   - Verify all links work
   - Ensure CTAs are prominent

2. **Add Visuals**
   - Add hero images to static/
   - Add trust badges/logos if needed
   - Rebuild with `seite build`

3. **Final SEO Check**
   - Verify page appears in sitemap.xml
   - Check Open Graph tags render correctly
   - Validate structured data if applicable

## Rollback

If issues are found after deploying:

1. Add `draft: true` to the page frontmatter
2. Run `seite build` and `seite deploy` to remove from live site
3. Fix issues in the markdown file
4. Re-run `/landing-audit` to verify score
5. Re-publish with `/landing-publish`

## Integration with Other Commands

**Typical Workflow:**
```bash
# 1. Research (optional)
/landing-research "product hosting" --type seo

# 2. Create landing page
/landing-write "product hosting" --type seo --goal trial

# 3. Move to content/pages/ with proper frontmatter
# (landing-write saves to landing-pages/, move to content/pages/)

# 4. Audit the draft
/landing-audit content/pages/product-hosting.md

# 5. Fix any issues (if needed)
# Edit the file manually

# 6. Re-audit until score >=75
/landing-audit content/pages/product-hosting.md

# 7. Publish
/landing-publish content/pages/product-hosting.md
```
