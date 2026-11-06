export type Scale='linear'|'log'
export interface BaseControl {id:string;label:string;kind:string;unit?:string;read_only?:boolean}
export interface RangeControl extends BaseControl {kind:'fader'|'knob';min:number;max:number;scale:Scale;value:number}
export interface StepControl extends BaseControl {kind:'step_sequencer';step_count:number;steps:boolean[];position:number|null}
export interface TriggerControl extends BaseControl {kind:'trigger';serial:number}
export type ControlDefinition=RangeControl|StepControl|TriggerControl
export interface Plane {id:string;label:string;layout?:string;roles?:string[];read_only?:boolean;controls:ControlDefinition[]}
export interface Snapshot {type:'state.snapshot'|'state.updated';epoch?:string;revision:number;schema_revision?:number;state:{name:string;role?:string|null;planes?:Plane[];values?:Record<string,unknown>;controls?:Record<string,unknown>}}
export type Command={type:'control.set';name:string;value:number|boolean|string}|{type:'step.toggle';plane:string;control:string;step:number}|{type:'trigger.fire';plane:string;control:string}
