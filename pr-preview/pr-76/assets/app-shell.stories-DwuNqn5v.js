import{j as e}from"./jsx-runtime-u17CrQMm.js";import{R as c,c as p,a as m,b as u,d as x}from"./RouterProvider-BqHI7qVd.js";import{r as h}from"./iframe-B-GtSnBd.js";import{S as f,A as j,a as N,b as g}from"./app-sidebar-Bxwk2bx2.js";import{M as S,C as R}from"./chat-sidebar-CUraSjv9.js";import{B as b}from"./button-DvKIUMmd.js";import{S as y}from"./separator-CO3ALU5n.js";import{T as v}from"./tooltip-BUFmSNEF.js";import"./index-9aaNveEi.js";import"./index-Dcbo3Ch8.js";import"./index-Bkq8jyEK.js";import"./preload-helper-PPVm8Dsz.js";import"./input-6vPGTOQc.js";import"./utils-CiB0LXSo.js";import"./x-D_KKK9vu.js";import"./createLucideIcon-BD2Ip9-i.js";import"./index-CC263THr.js";import"./index-C1pT-9O9.js";import"./index-afED5R8w.js";import"./index-s-8LXRW1.js";import"./skeleton-CdZfMvcJ.js";import"./history--CRgRhAi.js";function l({children:r}){const[t,s]=h.useState(!1);return e.jsx(v,{children:e.jsxs(f,{children:[e.jsx(j,{}),e.jsxs(N,{children:[e.jsxs("header",{className:"flex h-14 shrink-0 items-center gap-2 border-b px-4",children:[e.jsx(g,{className:"-ml-1"}),e.jsx(y,{orientation:"vertical",className:"mr-2 !h-4"}),e.jsx("h1",{className:"text-lg font-semibold",children:"T-Rader"}),e.jsx(b,{variant:"ghost",size:"icon",className:"ml-auto size-7",onClick:()=>s(i=>!i),"aria-label":"AI チャット","aria-expanded":t,children:e.jsx(S,{className:"size-4"})})]}),e.jsx("div",{className:"flex-1 p-4",children:r})]}),e.jsx(R,{isOpen:t,onClose:()=>s(!1)})]})})}l.__docgenInfo={description:"",methods:[],displayName:"AppShell",props:{children:{required:!0,tsType:{name:"ReactNode"},description:""}}};function d(r){const t=p({component:()=>e.jsx(l,{children:r})}),s=m({getParentRoute:()=>t,path:"/",component:()=>null}),i=m({getParentRoute:()=>t,path:"/charts/$instrumentId",component:()=>null});return u({routeTree:t.addChildren([s,i]),history:x({initialEntries:["/"]})})}const K={title:"Components/AppShell",parameters:{layout:"fullscreen"}},o={render:()=>{const r=d(e.jsxs("div",{className:"space-y-4",children:[e.jsx("h2",{className:"text-xl font-bold",children:"ウォッチリスト"}),e.jsx("p",{className:"text-muted-foreground",children:"ここにウォッチリストの内容が表示されます。"})]}));return e.jsx(c,{router:r})}},a={render:()=>{const r=d(e.jsxs("div",{className:"space-y-4",children:[e.jsx("h2",{className:"text-xl font-bold",children:"ウォッチリスト"}),Array.from({length:50},(t,s)=>e.jsxs("p",{className:"text-muted-foreground",children:["アイテム ",s+1,": サンプルコンテンツ"]},s))]}));return e.jsx(c,{router:r})}},n={render:()=>{const r=d(e.jsxs("div",{className:"space-y-4",children:[e.jsx("h2",{className:"text-xl font-bold",children:"ウォッチリスト"}),e.jsx("p",{className:"text-muted-foreground",children:"右上の AI チャットボタンをクリックして、サイドバーの開閉を確認できます。"})]}));return e.jsx(c,{router:r})}};o.parameters={...o.parameters,docs:{...o.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter(<div className="space-y-4">
        <h2 className="text-xl font-bold">ウォッチリスト</h2>
        <p className="text-muted-foreground">
          ここにウォッチリストの内容が表示されます。
        </p>
      </div>);
    return <RouterProvider router={router} />;
  }
}`,...o.parameters?.docs?.source}}};a.parameters={...a.parameters,docs:{...a.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter(<div className="space-y-4">
        <h2 className="text-xl font-bold">ウォッチリスト</h2>
        {Array.from({
        length: 50
      }, (_, i) => <p key={i} className="text-muted-foreground">
            アイテム {i + 1}: サンプルコンテンツ
          </p>)}
      </div>);
    return <RouterProvider router={router} />;
  }
}`,...a.parameters?.docs?.source}}};n.parameters={...n.parameters,docs:{...n.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter(<div className="space-y-4">
        <h2 className="text-xl font-bold">ウォッチリスト</h2>
        <p className="text-muted-foreground">
          右上の AI
          チャットボタンをクリックして、サイドバーの開閉を確認できます。
        </p>
      </div>);
    return <RouterProvider router={router} />;
  }
}`,...n.parameters?.docs?.source}}};const Q=["Default","WithLongContent","WithChatSidebar"];export{o as Default,n as WithChatSidebar,a as WithLongContent,Q as __namedExportsOrder,K as default};
