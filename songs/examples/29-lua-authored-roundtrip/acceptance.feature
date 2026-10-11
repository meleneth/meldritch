@design @headless
Feature: Lua reconstructs accepted authored state
  Scenario: Smallest parameter round-trip
    Given a temporary immutable base from example "02-synth-parameter-pattern"
    When source low-pass cutoff is set to 900 Hz and exported as authored Lua
    And a fresh host loads the Lua export with no session journal or hardware
    Then the same synth and module identities have source cutoff 900 Hz
    And compilation succeeds and deterministic source fingerprint and audio match
    And saving the unchanged source again produces identical Lua text

  Scenario: Composed authoring round-trip
    Given accepted synth, note, duration, mapping, mixer and patch edits from examples 25 to 30
    When those edits are exported as authored Lua
    And a fresh host loads the Lua export with no session journal or hardware
    Then all source identities, musical events, cables, mappings and lane levels match
    And whole-range and split-range rendered audio match the pre-save revision
    And no transport position, held pads or device availability is restored as music

  Scenario Outline: Failed save preserves recoverability
    Given a previous good Lua export and unsaved accepted edits
    When save fails at "<boundary>"
    Then the previous good export remains loadable and unchanged
    And the error is displayed and the current authored state remains dirty for retry
    Examples:
      | boundary       |
      | temporary write|
      | sync           |
      | atomic replace |

  Scenario Outline: Reject invalid reload or lossy export
    When persistence encounters "<invalid>"
    Then a source-aware diagnostic rejects it before musical mutation or file replacement
    Examples:
      | invalid                    |
      | base reference escapes root|
      | missing base               |
      | changed base fingerprint   |
      | duplicate source identity  |
      | unknown authoring version  |
      | unsupported source edit    |
      | omitted semantic content   |
