import { app } from './store.svelte';

/** What the library passes to cards/rows while a playlist is being viewed. */
export interface PlaylistCtx {
  /** True only when the visible order IS the playlist order (Playlist order, ascending). */
  canReorder: boolean;
  onmove: (draggedId: number, targetId: number, after: boolean) => void;
  onremove: (videoId: number) => void;
}

function reset() {
  app.dnd.dragging = null;
  app.dnd.target = null;
}

/** HTML5 drag-and-drop handlers for one reorderable item. `axis` = the direction the items flow in. */
export function dragHandlers(id: number, axis: 'x' | 'y', onmove: PlaylistCtx['onmove']) {
  return {
    ondragstart: (e: DragEvent) => {
      app.dnd.dragging = id;
      if (e.dataTransfer) {
        e.dataTransfer.setData('text/plain', String(id));
        e.dataTransfer.effectAllowed = 'move';
      }
    },
    ondragover: (e: DragEvent) => {
      if (app.dnd.dragging === null) return;
      e.preventDefault();
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      app.dnd.target = id;
      app.dnd.after = axis === 'x' ? e.clientX > r.left + r.width / 2 : e.clientY > r.top + r.height / 2;
    },
    ondrop: (e: DragEvent) => {
      e.preventDefault();
      const dragged = app.dnd.dragging;
      const after = app.dnd.after;
      reset();
      if (dragged !== null && dragged !== id) onmove(dragged, id, after);
    },
    ondragend: reset,
  };
}

/** CSS class for the drop indicator on an item. */
export function dropMark(id: number): '' | 'drop-before' | 'drop-after' {
  if (app.dnd.dragging === null || app.dnd.target !== id || app.dnd.dragging === id) return '';
  return app.dnd.after ? 'drop-after' : 'drop-before';
}
