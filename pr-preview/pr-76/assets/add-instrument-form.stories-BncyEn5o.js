import{j as n}from"./jsx-runtime-u17CrQMm.js";import"./iframe-B-GtSnBd.js";import{B as I}from"./button-DvKIUMmd.js";import{I as d}from"./input-6vPGTOQc.js";import{L as v}from"./client-FJsltP3e.js";import{P as f}from"./plus-CHCdJBDK.js";import"./preload-helper-PPVm8Dsz.js";import"./utils-CiB0LXSo.js";import"./index-s-8LXRW1.js";import"./createLucideIcon-BD2Ip9-i.js";function l({instrumentId:e,name:i,onInstrumentIdChange:p,onNameChange:g,onSubmit:c,isSubmitting:r,error:u}){return n.jsxs("form",{onSubmit:c,className:"space-y-2",children:[n.jsxs("div",{className:"flex items-end gap-2",children:[n.jsx(d,{placeholder:"銘柄コード (例: 7203)",value:e,onChange:o=>p(o.target.value),disabled:r,className:"max-w-40"}),n.jsx(d,{placeholder:"銘柄名 (例: トヨタ自動車)",value:i,onChange:o=>g(o.target.value),disabled:r}),n.jsxs(I,{type:"submit",disabled:!e.trim()||!i.trim()||r,children:[r?n.jsx(v,{className:"size-4 animate-spin"}):n.jsx(f,{className:"size-4"}),"追加"]})]}),u&&n.jsx("p",{className:"text-sm text-destructive",children:u})]})}l.__docgenInfo={description:"",methods:[],displayName:"AddInstrumentFormView",props:{instrumentId:{required:!0,tsType:{name:"string"},description:""},name:{required:!0,tsType:{name:"string"},description:""},onInstrumentIdChange:{required:!0,tsType:{name:"signature",type:"function",raw:"(value: string) => void",signature:{arguments:[{type:{name:"string"},name:"value"}],return:{name:"void"}}},description:""},onNameChange:{required:!0,tsType:{name:"signature",type:"function",raw:"(value: string) => void",signature:{arguments:[{type:{name:"string"},name:"value"}],return:{name:"void"}}},description:""},onSubmit:{required:!0,tsType:{name:"signature",type:"function",raw:"(e: FormEvent) => void",signature:{arguments:[{type:{name:"FormEvent"},name:"e"}],return:{name:"void"}}},description:""},isSubmitting:{required:!0,tsType:{name:"boolean"},description:""},error:{required:!0,tsType:{name:"union",raw:"string | null",elements:[{name:"string"},{name:"null"}]},description:""}}};const E={title:"Components/AddInstrumentForm",component:l},t={args:{instrumentId:"",name:"",onInstrumentIdChange:()=>{},onNameChange:()=>{},onSubmit:e=>e.preventDefault(),isSubmitting:!1,error:null}},a={args:{instrumentId:"7203",name:"トヨタ自動車",onInstrumentIdChange:()=>{},onNameChange:()=>{},onSubmit:e=>e.preventDefault(),isSubmitting:!1,error:null}},s={args:{instrumentId:"7203",name:"トヨタ自動車",onInstrumentIdChange:()=>{},onNameChange:()=>{},onSubmit:e=>e.preventDefault(),isSubmitting:!0,error:null}},m={args:{instrumentId:"7203",name:"トヨタ自動車",onInstrumentIdChange:()=>{},onNameChange:()=>{},onSubmit:e=>e.preventDefault(),isSubmitting:!1,error:"この銘柄は既にウォッチリストに追加されています"}};t.parameters={...t.parameters,docs:{...t.parameters?.docs,source:{originalSource:`{
  args: {
    instrumentId: '',
    name: '',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: false,
    error: null
  }
}`,...t.parameters?.docs?.source}}};a.parameters={...a.parameters,docs:{...a.parameters?.docs,source:{originalSource:`{
  args: {
    instrumentId: '7203',
    name: 'トヨタ自動車',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: false,
    error: null
  }
}`,...a.parameters?.docs?.source}}};s.parameters={...s.parameters,docs:{...s.parameters?.docs,source:{originalSource:`{
  args: {
    instrumentId: '7203',
    name: 'トヨタ自動車',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: true,
    error: null
  }
}`,...s.parameters?.docs?.source}}};m.parameters={...m.parameters,docs:{...m.parameters?.docs,source:{originalSource:`{
  args: {
    instrumentId: '7203',
    name: 'トヨタ自動車',
    onInstrumentIdChange: () => {},
    onNameChange: () => {},
    onSubmit: (e: FormEvent) => e.preventDefault(),
    isSubmitting: false,
    error: 'この銘柄は既にウォッチリストに追加されています'
  }
}`,...m.parameters?.docs?.source}}};const q=["Default","Filled","Submitting","WithError"];export{t as Default,a as Filled,s as Submitting,m as WithError,q as __namedExportsOrder,E as default};
