import{j as r}from"./jsx-runtime-u17CrQMm.js";function s({error:n,resetErrorBoundary:t}){return r.jsx("div",{className:"flex h-screen items-center justify-center",children:r.jsxs("div",{className:"max-w-md text-center",children:[r.jsx("h1",{className:"text-2xl font-bold text-destructive",children:"エラーが発生しました"}),r.jsx("p",{className:"mt-2 text-muted-foreground",children:n instanceof Error?n.message:"予期しないエラーが発生しました"}),r.jsx("button",{type:"button",onClick:t,className:"mt-4 rounded-md bg-primary px-4 py-2 text-primary-foreground hover:bg-primary/90",children:"再試行"})]})})}s.__docgenInfo={description:"",methods:[],displayName:"ErrorFallback"};const{fn:a}=__STORYBOOK_MODULE_TEST__,m={title:"Components/ErrorFallback",component:s,args:{resetErrorBoundary:a()}},e={args:{error:new Error("データの取得に失敗しました")}},o={args:{error:"unknown error"}};e.parameters={...e.parameters,docs:{...e.parameters?.docs,source:{originalSource:`{
  args: {
    error: new Error('データの取得に失敗しました')
  }
}`,...e.parameters?.docs?.source}}};o.parameters={...o.parameters,docs:{...o.parameters?.docs,source:{originalSource:`{
  args: {
    error: 'unknown error'
  }
}`,...o.parameters?.docs?.source}}};const d=["Default","UnknownError"];export{e as Default,o as UnknownError,d as __namedExportsOrder,m as default};
