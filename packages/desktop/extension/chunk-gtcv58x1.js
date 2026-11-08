var e;function o(){function a(){let t=e?.[0].matches,c=e?.[1].matches;document.documentElement.dataset.theme=`${t?"dark":"light"}${c?"-contrast":""}`}if(!e){e=[matchMedia("(prefers-color-scheme: dark)"),matchMedia("(prefers-contrast: more)")];for(let t of e)t.addEventListener("change",a)}a()}
export{o};
