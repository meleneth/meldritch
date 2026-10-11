@headless
Feature: Edit loaded synth source through the TUI
  Background:
    # Executable bindings: crates/meldritch-control/tests/instrument_authoring.rs
    Given a temporary copy of example "25-edit-existing-synth"
    And the loaded song is playing and its source revision is inspected

  Scenario Outline: Edit a source parameter and reload it
    When the TUI sets module kind "<module>" parameter "<parameter>" to <value>
    Then the source inspector shows <value> in "<unit>" at the same module identity
    And exactly one changed typed command is accepted and recorded
    And the source fingerprint changes and dependent audio is invalidated
    And completed worker audio differs from the prior render and remains finite
    And all surfaces distinguish requested, published and callback-consumed revisions
    When authored edits are saved as Lua and reloaded into a fresh host
    Then the edited source identities, values and deterministic rendered audio match
    Examples:
      | module   | parameter | value | unit    |
      | low_pass | cutoff_hz | 600   | Hz      |
      | adsr     | release   | 0.4   | seconds |

  Scenario Outline: Reject invalid edits without side effects
    When the TUI attempts source edit "<edit>"
    Then a diagnostic identifies the target and invalid value
    And source fingerprint, accepted journal and published audio remain unchanged
    Examples:
      | edit                    |
      | missing-module.cutoff=1 |
      | low_pass.cutoff=NaN     |
      | adsr.release=-0.1       |
