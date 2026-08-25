# Vanta OBS Companion

This package is intentionally small. Vanta Live Studio remains the primary production UI; this OBS companion is only for creators who keep OBS Studio in their workflow.

## Install

1. Add `vanta_obs_bridge.lua` in OBS `Tools -> Scripts`.
2. Configure the Vanta API base URL, broadcast ID, user ID, role, and bearer token.
3. Click `Open Vanta Dock URL`.
4. Add the printed URL as an OBS Custom Browser Dock.

## Scope

- Authenticate to Vanta with a bearer token and Vanta role headers.
- Show compact stream health and archive status.
- Trigger sponsor cues.
- Capture sponsor proof markers.
- Save replay markers.
- Sync live and archive state from Vanta.

This companion does not expose the full Vanta studio, generic plugin hosting, scene editing, or encoder tuning inside OBS.
