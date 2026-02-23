import{j as t}from"./jsx-runtime-u17CrQMm.js";import{R as i,c as u,a as c,b as l,d as p}from"./RouterProvider-BqHI7qVd.js";import{a}from"./watchlist-item-row-D6WRD8jB.js";import"./iframe-B-GtSnBd.js";import"./preload-helper-PPVm8Dsz.js";import"./index-9aaNveEi.js";import"./index-Dcbo3Ch8.js";import"./index-Bkq8jyEK.js";import"./client-FJsltP3e.js";import"./createLucideIcon-BD2Ip9-i.js";import"./button-DvKIUMmd.js";import"./utils-CiB0LXSo.js";import"./index-s-8LXRW1.js";import"./trash-2-DliGacO7.js";function n(e){const d=u({component:()=>e}),m=c({getParentRoute:()=>d,path:"/charts/$instrumentId",component:()=>t.jsx("div",{children:"チャート画面"})});return l({routeTree:d.addChildren([m]),history:p({initialEntries:["/"]})})}const I={title:"Components/WatchlistItemRow"},r={render:()=>{const e=n(t.jsx(a,{item:{watchlist_id:"123",instrument_id:"7203",sort_order:0,added_at:"2026-01-01T00:00:00Z"},name:"トヨタ自動車",isDeleting:!1,onDelete:()=>{}}));return t.jsx(i,{router:e})}},o={render:()=>{const e=n(t.jsx(a,{item:{watchlist_id:"123",instrument_id:"9984",sort_order:1,added_at:"2026-01-01T00:00:00Z"},name:void 0,isDeleting:!1,onDelete:()=>{}}));return t.jsx(i,{router:e})}},s={render:()=>{const e=n(t.jsx(a,{item:{watchlist_id:"123",instrument_id:"7203",sort_order:0,added_at:"2026-01-01T00:00:00Z"},name:"トヨタ自動車",isDeleting:!0,onDelete:()=>{}}));return t.jsx(i,{router:e})}};r.parameters={...r.parameters,docs:{...r.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter(<WatchlistItemRowView item={{
      watchlist_id: '123',
      instrument_id: '7203',
      sort_order: 0,
      added_at: '2026-01-01T00:00:00Z'
    }} name="トヨタ自動車" isDeleting={false} onDelete={() => {}} />);
    return <RouterProvider router={router} />;
  }
}`,...r.parameters?.docs?.source}}};o.parameters={...o.parameters,docs:{...o.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter(<WatchlistItemRowView item={{
      watchlist_id: '123',
      instrument_id: '9984',
      sort_order: 1,
      added_at: '2026-01-01T00:00:00Z'
    }} name={undefined} isDeleting={false} onDelete={() => {}} />);
    return <RouterProvider router={router} />;
  }
}`,...o.parameters?.docs?.source}}};s.parameters={...s.parameters,docs:{...s.parameters?.docs,source:{originalSource:`{
  render: () => {
    const router = createStoryRouter(<WatchlistItemRowView item={{
      watchlist_id: '123',
      instrument_id: '7203',
      sort_order: 0,
      added_at: '2026-01-01T00:00:00Z'
    }} name="トヨタ自動車" isDeleting={true} onDelete={() => {}} />);
    return <RouterProvider router={router} />;
  }
}`,...s.parameters?.docs?.source}}};const Z=["Default","WithoutName","Deleting"];export{r as Default,s as Deleting,o as WithoutName,Z as __namedExportsOrder,I as default};
