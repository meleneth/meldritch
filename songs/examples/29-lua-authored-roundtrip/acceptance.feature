@headless @rust_project_authoring
Feature: Return to composed music
  Scenario: Create change publish save exit and fresh-process reload
    Given example 29 loaded with its synth and Lua controller policy
    When keyboard commands edit sound and patch connections
    And APC presses create notes and held durations on two time pages
    And assignments and CC pickup edit cutoff volume and pan
    And all views and simulated surfaces observe authoritative music
    And composed Lua is saved and the performer exits cleanly
    Then a fresh process restores music without reading old journals
    And Lua text source state and deterministic f64 audio are identical

  Scenario: Invalid commands preserve source history and publication
    When a missing parameter wrong type invalid range or unsupported target is edited
    Then the typed command rejects without changing music

  Scenario: Interrupted gesture consumes late releases
    When APC disconnect interrupts a held duration gesture
    Then late note-off and zero-velocity releases create no unintended note
    And the next gesture composes normally

  Scenario: Failed save preserves recoverability
    Given previous valid Lua and unsaved music
    When the authored directory is blocked by a file
    Then all editor views report failed save and unsaved edits
    And prior Lua journal and authoritative music remain intact

  Scenario: Stale graph draft cannot replace current source
    When music changes after a graph draft opens
    Then stale commit rejects without source replacement

  Scenario: Invalid reload and lossy export reject
    When a component has invalid version duration cable binding value or base
    Then semantic validation rejects it
    And export of two edited patterns rejects omitted content
