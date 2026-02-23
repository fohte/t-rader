import{I as t}from"./input-6vPGTOQc.js";import"./jsx-runtime-u17CrQMm.js";import"./iframe-B-GtSnBd.js";import"./preload-helper-PPVm8Dsz.js";import"./utils-CiB0LXSo.js";const i={title:"UI/Input",component:t,argTypes:{type:{control:"select",options:["text","email","password","number","search"]},disabled:{control:"boolean"},placeholder:{control:"text"}}},e={args:{placeholder:"テキストを入力..."}},r={args:{defaultValue:"入力済みテキスト"}},a={args:{type:"password",placeholder:"パスワード"}},s={args:{disabled:!0,placeholder:"無効な入力欄"}},o={args:{type:"file"}};e.parameters={...e.parameters,docs:{...e.parameters?.docs,source:{originalSource:`{
  args: {
    placeholder: 'テキストを入力...'
  }
}`,...e.parameters?.docs?.source}}};r.parameters={...r.parameters,docs:{...r.parameters?.docs,source:{originalSource:`{
  args: {
    defaultValue: '入力済みテキスト'
  }
}`,...r.parameters?.docs?.source}}};a.parameters={...a.parameters,docs:{...a.parameters?.docs,source:{originalSource:`{
  args: {
    type: 'password',
    placeholder: 'パスワード'
  }
}`,...a.parameters?.docs?.source}}};s.parameters={...s.parameters,docs:{...s.parameters?.docs,source:{originalSource:`{
  args: {
    disabled: true,
    placeholder: '無効な入力欄'
  }
}`,...s.parameters?.docs?.source}}};o.parameters={...o.parameters,docs:{...o.parameters?.docs,source:{originalSource:`{
  args: {
    type: 'file'
  }
}`,...o.parameters?.docs?.source}}};const m=["Default","WithValue","Password","Disabled","WithFile"];export{e as Default,s as Disabled,a as Password,o as WithFile,r as WithValue,m as __namedExportsOrder,i as default};
