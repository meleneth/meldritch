@accept @headless
Feature: Performer-authored fader assignments and mixer values
  Background:
    Given a temporary copy of example "28-author-fader-mappings"
    And main.lua declares typed targets, physical faders and two assignment pages

  Scenario: Observe, acquire, edit, publish and independently reload
    When scripts/compose.faders is executed through the production MIDI and Lua paths
    Then waiting observations produce no musical facts or audio generations
    And eleven accepted parameter observations produce seven changed generations
    And cutoff is 6000, resonance is 0.5, volume is 0.5, pan is 1 and delay mix is 1
    And playing published audio exactly matches independent final source rendering
    And APC and TUI projections observe the same source and assignments
    And saved Lua reload preserves mappings, values, fingerprint and exact audio
    And TOML, prior exports and performance journals remain unchanged

  Scenario: Change and remove assignments during performance
    When the TUI or authored APC controls change, remove or page a fader assignment
    Then pickup rearms before a supported typed edit is accepted
    And another source editor or observing fader change also rearms pickup
    And an unassigned fader updates observation telemetry without a musical edit

  Scenario: Preserve mono compatibility and explicit stereo balance
    Then untouched mono audio retains historical channel count and samples
    And zero track volume silences only that track
    And authored center pan duplicates mono at unity gain
    And hard pan silences the opposite stereo channel
    And native stereo retains separate channel content
    And split stereo ranges concatenate identically to the full render

  Scenario: Reject invalid assignments and preserve unsaved edits
    When invalid/*.lua and malformed declaration cases are exercised
    Then diagnostics reject missing, incompatible, ambiguous and invalid references
    And rejected commands preserve source, mappings and history
    And failed saves preserve unsaved edits and existing files
    And nested Lua projection mutations reject as read-only

# Executed by crates/meldritch-control/tests/assignments.rs and the assignments
# example runner; stereo balance unit coverage is in song_render::mixer_tests.
# Physical APC, LED and listening acceptance is explicitly pending.
