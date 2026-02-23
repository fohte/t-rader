import{j as e}from"./jsx-runtime-u17CrQMm.js";import{B as m}from"./button-DvKIUMmd.js";import{c as d}from"./utils-CiB0LXSo.js";import{X as c}from"./x-D_KKK9vu.js";import"./iframe-B-GtSnBd.js";import"./preload-helper-PPVm8Dsz.js";import"./index-s-8LXRW1.js";import"./createLucideIcon-BD2Ip9-i.js";function n({instrumentId:t,isOpen:a,onToggle:o,className:i}){return a?e.jsxs("div",{className:d("flex w-72 shrink-0 flex-col rounded-md border bg-card p-4",i),children:[e.jsxs("div",{className:"flex items-center justify-between",children:[e.jsx("h2",{className:"text-sm font-semibold",children:"板情報・歩み値"}),e.jsx(m,{variant:"ghost",size:"icon-xs",onClick:o,"aria-label":"パネルを閉じる",children:e.jsx(c,{})})]}),e.jsx("div",{className:"mt-4 flex flex-1 items-center justify-center",children:e.jsxs("p",{className:"text-sm text-muted-foreground",children:[t," - 開発中"]})})]}):null}n.__docgenInfo={description:"",methods:[],displayName:"ChartMarketDepthPanel",props:{instrumentId:{required:!0,tsType:{name:"string"},description:""},isOpen:{required:!0,tsType:{name:"boolean"},description:""},onToggle:{required:!0,tsType:{name:"signature",type:"function",raw:"() => void",signature:{arguments:[],return:{name:"void"}}},description:""},className:{required:!1,tsType:{name:"string"},description:""}}};const{fn:p}=__STORYBOOK_MODULE_TEST__,_={title:"Components/ChartMarketDepthPanel",component:n,decorators:[t=>e.jsx("div",{style:{height:"400px"},children:e.jsx(t,{})})],args:{onToggle:p()}},r={args:{instrumentId:"7203",isOpen:!0}},s={args:{instrumentId:"7203",isOpen:!1}};r.parameters={...r.parameters,docs:{...r.parameters?.docs,source:{originalSource:`{
  args: {
    instrumentId: '7203',
    isOpen: true
  }
}`,...r.parameters?.docs?.source}}};s.parameters={...s.parameters,docs:{...s.parameters?.docs,source:{originalSource:`{
  args: {
    instrumentId: '7203',
    isOpen: false
  }
}`,...s.parameters?.docs?.source}}};const y=["Open","Closed"];export{s as Closed,r as Open,y as __namedExportsOrder,_ as default};
