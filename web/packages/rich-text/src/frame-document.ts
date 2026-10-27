// All executable assets are same-origin files; Markdown must never enable eval.
export const VDITOR_FRAME_DOCUMENT = `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="script-src 'self'; object-src 'none'; base-uri 'self'"><style>
html,body{margin:0;padding:0;background:transparent;font-family:system-ui,sans-serif}*{box-sizing:border-box}
body>.vditor{border:0;border-radius:0}.vditor-reset{font-size:16px;line-height:1.8}.vditor-reset img{max-width:100%}
.oneflow-code-copy{position:absolute;right:8px;top:6px;border:1px solid #d5ddd8;border-radius:4px;background:white;color:#57605d;cursor:pointer;padding:2px 7px}
@media(max-width:520px){.vditor-toolbar{padding:3px!important}.vditor-toolbar>.vditor-toolbar__item{padding:0 3px}.vditor-ir pre.vditor-reset,.vditor-wysiwyg pre.vditor-reset,.vditor-sv{padding:12px!important}}
#preview{padding:0;overflow-wrap:anywhere;display:flow-root}#preview> :first-child{margin-top:0}#preview> :last-child{margin-bottom:0}
</style></head><body></body></html>`;
