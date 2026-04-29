# Settings i18n - Remaining Tasks

## Current Status

| Metric | Count |
|--------|-------|
| en.rs keys | 940 |
| zh_cn.rs keys | 841 (99 missing) |
| ja.rs keys | 635 (305 missing) |
| ko.rs keys | 635 (305 missing) |
| de.rs keys | 635 (305 missing) |
| pt_br.rs keys | 635 (305 missing) |
| `cargo check -p warp` | PASS |
| `cargo test -p i18n` | FAIL (key sync issue) |

## 1. Missing Translation Keys (HIGH PRIORITY)

The following pages had keys added to en.rs by background agents, but translations are missing in other languages:

### Missing from zh_cn.rs (99 keys)
- `settings.billing.*` (99 keys) - billing/usage page translations

### Missing from ja/ko/de/pt_br (305 keys each)
- `settings.teams.*` (109 keys) - teams page
- `settings.billing.*` (99 keys) - billing/usage page
- `settings.execution_profile.*` (30 keys) - execution profile
- `settings.main.*` (22 keys) - main page
- `settings.env_form.*` (21 keys) - environment form
- `settings.show_blocks.*` (11 keys) - show blocks
- `settings.warp_drive.*` (3 keys) - warp drive
- `settings.footer.*` (3 keys) - settings footer
- `settings.delete_env.*` (3 keys) - delete env dialog
- `settings.transfer.*` (2 keys) - transfer ownership
- `settings.dir_color.*` (2 keys) - directory color picker

**Action**: Add translations to zh_cn.rs, ja.rs, ko.rs, de.rs, pt_br.rs for all 305 keys.

## 2. Source Files Not Yet i18n-ized (0 i18n::t calls)

These settings_view files have NOT been converted to use i18n yet:

### High Priority (large files, many user-facing strings)
- [ ] `teams_page.rs` (~4097 lines) - team management UI
- [ ] `billing_and_usage_page.rs` (~3697 lines) - billing, usage, plans
- [ ] `execution_profile_view.rs` - execution profile settings
- [ ] `show_blocks_view.rs` - block display settings
- [ ] `main_page.rs` - main settings page

### Medium Priority
- [ ] `warp_drive_page.rs` - warp drive page
- [ ] `settings_file_footer.rs` - settings file footer
- [ ] `delete_environment_confirmation_dialog.rs` - delete env dialog
- [ ] `transfer_ownership_confirmation_modal.rs` - transfer ownership
- [ ] `directory_color_add_picker.rs` - directory color picker
- [ ] `update_environment_form.rs` - update env form

### Low Priority / Infrastructure (no user-facing strings)
- [ ] `about_page.rs` - internal IDs only
- [ ] `settings_page.rs` - infrastructure
- [ ] `telemetry.rs` - telemetry strings (not translated)
- [ ] `admin_actions.rs` - admin actions
- [ ] `tab_menu.rs` - tab menu
- [ ] `pane_manager.rs` - pane manager

## 3. Files Already i18n-ized (for reference)

| File | i18n::t() calls |
|------|----------------|
| ai_page.rs | 181 |
| features_page.rs | 119 |
| appearance_page.rs | 95 |
| code_page.rs | 58 |
| environments_page.rs | 40 |
| privacy_page.rs | 38 |
| mod.rs | 25 |
| platform_page.rs | 20 |
| warpify_page.rs | 19 |
| keybindings.rs | 15 |
| agent_assisted_environment_modal.rs | 14 |
| mcp_servers_page.rs | 6 |
| referrals_page.rs | 3 |
| nav.rs | 1 |
| external_editor.rs | (partial) |

## 4. Verification Checklist

After completing all above:
- [ ] `cargo test -p i18n` passes (all 11 tests)
- [ ] `cargo check -p warp` compiles
- [ ] All 940 keys present in all 6 language files
- [ ] No raw CJK/accented characters (all unicode escapes)
- [ ] Total i18n::t() calls across all settings files counted and reported

## Background Agents Status

- [DONE] environments_page.rs agent - added 38 keys + 40 i18n::t() calls
- [DONE] agent_assisted_environment_modal.rs agent - added 13 keys + 14 i18n::t() calls
- [RUNNING] teams_page.rs agent (a3157a1157e30d218)
- [RUNNING] billing_and_usage_page.rs agent (ad468665e82e9e602)
- [RUNNING] platform+mcp+warpify+referrals agent (a17315e09604129cd)
- [RUNNING] sub-pages and modals agent (af966451535c734b8)
