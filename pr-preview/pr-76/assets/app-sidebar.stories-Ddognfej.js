import{j as e}from"./jsx-runtime-u17CrQMm.js";import{R as c,c as P,a as o,b as g,d as S}from"./RouterProvider-BqHI7qVd.js";import{S as h,A as x,a as j}from"./app-sidebar-Bxwk2bx2.js";import"./iframe-B-GtSnBd.js";import"./preload-helper-PPVm8Dsz.js";import"./index-9aaNveEi.js";import"./index-Dcbo3Ch8.js";import"./index-Bkq8jyEK.js";import"./button-DvKIUMmd.js";import"./utils-CiB0LXSo.js";import"./index-s-8LXRW1.js";import"./input-6vPGTOQc.js";import"./separator-CO3ALU5n.js";import"./index-afED5R8w.js";import"./x-D_KKK9vu.js";import"./createLucideIcon-BD2Ip9-i.js";import"./index-CC263THr.js";import"./index-C1pT-9O9.js";import"./skeleton-CdZfMvcJ.js";import"./tooltip-BUFmSNEF.js";import"./history--CRgRhAi.js";function i(r,p){const t=P({component:()=>e.jsxs(h,{children:[e.jsx(x,{}),e.jsx(j,{children:e.jsx("div",{className:"p-4",children:p})})]})}),m=o({getParentRoute:()=>t,path:"/",component:()=>null}),d=o({getParentRoute:()=>t,path:"/charts/$instrumentId",component:()=>null}),l=o({getParentRoute:()=>t,path:"/trades",component:()=>null}),R=o({getParentRoute:()=>t,path:"/notes",component:()=>null});return g({routeTree:t.addChildren([m,d,l,R]),history:S({initialEntries:[r]})})}const B={title:"Components/AppSidebar",parameters:{layout:"fullscreen"}},n={render:()=>{const r=i("/","ページコンテンツ");return e.jsx(c,{router:r})}},s={render:()=>{const r=i("/","ウォッチリストページ");return e.jsx(c,{router:r})}},a={render:()=>{const r=i("/trades","トレード履歴ページ");return e.jsx(c,{router:r})}},u={render:()=>{const r=i("/notes","ノートページ");return e.jsx(c,{router:r})}};n.parameters={...n.parameters,docs:{...n.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter('/', 'ページコンテンツ');
    return <RouterProvider router={router} />;
  }
}`,...n.parameters?.docs?.source}}};s.parameters={...s.parameters,docs:{...s.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter('/', 'ウォッチリストページ');
    return <RouterProvider router={router} />;
  }
}`,...s.parameters?.docs?.source}}};a.parameters={...a.parameters,docs:{...a.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter('/trades', 'トレード履歴ページ');
    return <RouterProvider router={router} />;
  }
}`,...a.parameters?.docs?.source}}};u.parameters={...u.parameters,docs:{...u.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter('/notes', 'ノートページ');
    return <RouterProvider router={router} />;
  }
}`,...u.parameters?.docs?.source}}};const F=["Default","OnWatchlistPage","OnTradesPage","OnNotesPage"];export{n as Default,u as OnNotesPage,a as OnTradesPage,s as OnWatchlistPage,F as __namedExportsOrder,B as default};
