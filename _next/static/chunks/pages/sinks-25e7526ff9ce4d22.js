(self.webpackChunk_N_E=self.webpackChunk_N_E||[]).push([[893],{83066:function(n,e,i){(window.__NEXT_P=window.__NEXT_P||[]).push(["/sinks",function(){return i(5002)}])},3585:function(n,e,i){"use strict";i.d(e,{D:function(){return h},r:function(){return u}});var r=i(85893),t=i(79351),l=i(47741),d=i(41664),c=i.n(d),a=i(95100),o=i(33256),s=i(40712);function u(n){let[e,i]=(0,a.v1)("modalId",a.U);return[null==n?void 0:n.find(n=>n.id===e),i]}function h(n){let{modalData:e,onClose:i}=n;return(0,r.jsxs)(t.u_,{isOpen:void 0!==e,onClose:i,size:"3xl",children:[(0,r.jsx)(t.ZA,{}),(0,r.jsxs)(t.hz,{children:[(0,r.jsxs)(t.xB,{children:["Catalog of ",e&&(0,o.ks)(e)," ",null==e?void 0:e.id," - ",null==e?void 0:e.name]}),(0,r.jsx)(t.ol,{}),(0,r.jsx)(t.fe,{children:e&&(0,r.jsx)(s.Rm,{src:e,collapsed:1,name:null,displayDataTypes:!1})}),(0,r.jsxs)(t.mz,{children:[e&&(0,o.vx)(e)&&(0,r.jsx)(l.zx,{colorScheme:"blue",mr:3,children:(0,r.jsx)(c(),{href:"/fragment_graph/?id=".concat(e.id),children:"View Fragments"})}),(0,r.jsx)(l.zx,{mr:3,onClick:i,children:"Close"})]})]})]})}},40712:function(n,e,i){"use strict";i.d(e,{Rm:function(){return p},KB:function(){return C},Kf:function(){return _},gU:function(){return k},vk:function(){return w},sW:function(){return b},v6:function(){return T}});var r=i(85893),t=i(47741),l=i(40639),d=i(36696),c=i(63679),a=i(9008),o=i.n(a),s=i(41664),u=i.n(s),h=i(67294),m=i(64030),x=i(44599),j=i(33256);function f(n){var e,i,r,t;return"columnDesc"in n?"".concat(null===(e=n.columnDesc)||void 0===e?void 0:e.name," (").concat(null===(r=n.columnDesc)||void 0===r?void 0:null===(i=r.columnType)||void 0===i?void 0:i.typeName,")"):"".concat(n.name," (").concat(null===(t=n.dataType)||void 0===t?void 0:t.typeName,")")}var v=i(3585);let p=(0,c.ZP)(()=>i.e(171).then(i.t.bind(i,55171,23))),w={name:"Depends",width:1,content:n=>(0,r.jsx)(u(),{href:"/relation_graph/?id=".concat(n.id),children:(0,r.jsx)(t.zx,{size:"sm","aria-label":"view dependents",colorScheme:"blue",variant:"link",children:"D"})})},T=[{name:"Primary Key",width:1,content:n=>n.pk.map(n=>n.columnIndex).map(e=>n.columns[e]).map(n=>f(n)).join(", ")},{name:"Vnode Count",width:1,content:n=>{var e;return null!==(e=n.maybeVnodeCount)&&void 0!==e?e:"?"}}],k={name:"Connector",width:3,content:n=>{var e;return null!==(e=n.withProperties.connector)&&void 0!==e?e:"unknown"}},_={name:"Connector",width:3,content:n=>{var e;return null!==(e=n.properties.connector)&&void 0!==e?e:"unknown"}},b=[w,{name:"Fragments",width:1,content:n=>(0,r.jsx)(u(),{href:"/fragment_graph/?id=".concat(n.id),children:(0,r.jsx)(t.zx,{size:"sm","aria-label":"view fragments",colorScheme:"blue",variant:"link",children:"F"})})}];function C(n,e,i){let{response:c}=(0,x.Z)(async()=>{let n=await e(),i=await (0,j.Rf)(),r=await (0,j.Cp)(),t=await (0,j.jW)();return n.map(n=>{let e=i.find(e=>e.id===n.owner),l=null==e?void 0:e.name,d=t.find(e=>e.id===n.schemaId),c=null==d?void 0:d.name,a=r.find(e=>e.id===n.databaseId),o=null==a?void 0:a.name;return{...n,ownerName:l,schemaName:c,databaseName:o}})}),[a,s]=(0,v.r)(c),u=(0,r.jsx)(v.D,{modalData:a,onClose:()=>s(null)}),p=(0,r.jsxs)(l.xu,{p:3,children:[(0,r.jsx)(m.Z,{children:n}),(0,r.jsx)(d.xJ,{children:(0,r.jsxs)(d.iA,{variant:"simple",size:"sm",maxWidth:"full",children:[(0,r.jsx)(d.hr,{children:(0,r.jsxs)(d.Tr,{children:[(0,r.jsx)(d.Th,{width:3,children:"Id"}),(0,r.jsx)(d.Th,{width:5,children:"Database"}),(0,r.jsx)(d.Th,{width:5,children:"Schema"}),(0,r.jsx)(d.Th,{width:5,children:"Name"}),(0,r.jsx)(d.Th,{width:3,children:"Owner"}),i.map(n=>(0,r.jsx)(d.Th,{width:n.width,children:n.name},n.name)),(0,r.jsx)(d.Th,{children:"Visible Columns"})]})}),(0,r.jsx)(d.p3,{children:null==c?void 0:c.map(n=>(0,r.jsxs)(d.Tr,{children:[(0,r.jsx)(d.Td,{children:(0,r.jsx)(t.zx,{size:"sm","aria-label":"view catalog",colorScheme:"blue",variant:"link",onClick:()=>s(n.id),children:n.id})}),(0,r.jsx)(d.Td,{children:n.databaseName}),(0,r.jsx)(d.Td,{children:n.schemaName}),(0,r.jsx)(d.Td,{children:n.name}),(0,r.jsx)(d.Td,{children:n.ownerName}),i.map(e=>(0,r.jsx)(d.Td,{children:e.content(n)},e.name)),n.columns&&n.columns.length>0&&(0,r.jsx)(d.Td,{overflowWrap:"normal",children:n.columns.filter(n=>!("isHidden"in n)||!n.isHidden).map(n=>f(n)).join(", ")})]},n.id))})]})})]});return(0,r.jsxs)(h.Fragment,{children:[(0,r.jsx)(o(),{children:(0,r.jsx)("title",{children:n})}),u,p]})}},5002:function(n,e,i){"use strict";i.r(e),i.d(e,{default:function(){return l}});var r=i(40712),t=i(33256);function l(){return(0,r.KB)("Sinks",t.Iw,[r.Kf,...r.sW])}}},function(n){n.O(0,[662,679,184,876,278,721,371,888,774,179],function(){return n(n.s=83066)}),_N_E=n.O()}]);