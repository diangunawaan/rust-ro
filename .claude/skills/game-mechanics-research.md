---
name: ragnarok-pre-renewal-research
description: Research game mechanics and reference implementations for Ragnarok Online pre-renewal. Use when searching for RO game mechanics, formulas, skill behavior, monster data, item effects, or server implementation details. Triggers on questions about rAthena, Hercules, iRO classic, or any pre-renewal gameplay mechanics. Always uses Haiku model via subagent. Never includes renewal content.
---

# Ragnarok Online Pre-Renewal Research

## Model Usage
Always dispatch research tasks to a subagent using `claude-haiku-4-5-20251001`.

## Approved Web Resources
Search these sources for game mechanics and implementation details:

| Source | Use For |
|--------|---------|
| https://ro.kokotewa.com | Game data, item/monster databases |
| https://irowiki.org/classic | Pre-renewal wiki, skill descriptions, quests |
| https://github.com/HerculesWS/Hercules/wiki | Hercules server documentation |
| https://ragnarokresearchlab.github.io/game-mechanics | Detailed mechanics research |

## Local Reference Implementation
Check these first before searching the web:

- **Source code:** `../rathena`
- **Documentation:** `../rathena/doc`

## Research Priority
1. `../rathena/doc` — check for existing documentation
2. `../rathena` source code — implementation details
3. Approved web resources — additional context and verification

## Critical Constraints

**NEVER include information from:**
- Renewal mechanics or formulas
- Sources labeled "RE", "Renewal", or "post-renewal"
- Renewal-specific job classes, skills, or balance changes
- Any content dated after the renewal update (2010+) unless explicitly marked as classic/pre-renewal

When a source contains both pre-renewal and renewal content, extract **only** the pre-renewal information and explicitly note this distinction.

## Output Guidelines
- Cite specific sources (file path or URL)
- Include relevant code snippets from rathena when applicable
- Flag any inconsistencies between sources
- Note confidence level when information conflicts