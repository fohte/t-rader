import{j as s}from"./jsx-runtime-u17CrQMm.js";import{S as t}from"./separator-CO3ALU5n.js";import"./iframe-B-GtSnBd.js";import"./preload-helper-PPVm8Dsz.js";import"./utils-CiB0LXSo.js";import"./index-afED5R8w.js";import"./index-9aaNveEi.js";import"./index-Dcbo3Ch8.js";import"./index-s-8LXRW1.js";const N={title:"UI/Separator",component:t,argTypes:{orientation:{control:"select",options:["horizontal","vertical"]}}},r={args:{orientation:"horizontal"},decorators:[e=>s.jsxs("div",{className:"w-64",children:[s.jsx("p",{className:"text-sm",children:"上のコンテンツ"}),s.jsx(e,{}),s.jsx("p",{className:"text-sm",children:"下のコンテンツ"})]})]},a={args:{orientation:"vertical"},decorators:[e=>s.jsxs("div",{className:"flex h-8 items-center gap-4",children:[s.jsx("span",{className:"text-sm",children:"左"}),s.jsx(e,{}),s.jsx("span",{className:"text-sm",children:"右"})]})]};r.parameters={...r.parameters,docs:{...r.parameters?.docs,source:{originalSource:`{
  args: {
    orientation: 'horizontal'
  },
  decorators: [Story => <div className="w-64">
        <p className="text-sm">上のコンテンツ</p>
        <Story />
        <p className="text-sm">下のコンテンツ</p>
      </div>]
}`,...r.parameters?.docs?.source}}};a.parameters={...a.parameters,docs:{...a.parameters?.docs,source:{originalSource:`{
  args: {
    orientation: 'vertical'
  },
  decorators: [Story => <div className="flex h-8 items-center gap-4">
        <span className="text-sm">左</span>
        <Story />
        <span className="text-sm">右</span>
      </div>]
}`,...a.parameters?.docs?.source}}};const h=["Horizontal","Vertical"];export{r as Horizontal,a as Vertical,h as __namedExportsOrder,N as default};
