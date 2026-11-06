import {describe, expect, it} from 'vitest'
import {mount} from '@vue/test-utils'
import ControlRenderer from './ControlRenderer.vue'
import FaderControl from './controls/FaderControl.vue'

describe('control registry', () => {
  it('renders and dispatches sequencer edits', async () => {
    const wrapper = mount(ControlRenderer, {props: {control: {
      id: 'kick', label: 'Kick', kind: 'step_sequencer', step_count: 16,
      steps: Array.from({length: 16}, (_, index) => index === 0), position: 3,
    }}})
    expect(wrapper.findAll('button')).toHaveLength(16)
    await wrapper.findAll('button')[5].trigger('click')
    expect(wrapper.emitted('toggle')?.[0]).toEqual([5])
  })

  it('keeps triggers as repeatable events', async () => {
    const wrapper = mount(ControlRenderer, {props: {control: {
      id: 'fill', label: 'FILL', kind: 'trigger', serial: 2,
    }}})
    await wrapper.get('button').trigger('pointerdown')
    await wrapper.get('button').trigger('pointerup')
    await wrapper.get('button').trigger('pointerdown')
    expect(wrapper.emitted('fire')).toHaveLength(2)
  })

  it('maps a logarithmic fader and delivers its final value', async () => {
    const wrapper = mount(FaderControl, {props: {control: {
      id: 'cutoff', label: 'Cutoff', kind: 'fader', min: 20, max: 20000,
      scale: 'log', value: 20,
    }}})
    const input = wrapper.get('input')
    ;(input.element as HTMLInputElement).value = '0.5'
    await input.trigger('change')
    expect(wrapper.emitted('change')?.[0][0]).toBeCloseTo(Math.sqrt(20 * 20000))
  })
})
