"""Extract Blender materials and packed albedo maps for the CPU renderer."""
from pathlib import Path
import numpy as np

def export_materials(mesh, directory):
 dest=Path(directory)
 dest.mkdir(parents=True,exist_ok=True)
 rows=[];saved={}
 for i,mat in enumerate(mesh.materials):
  p=next((n for n in mat.node_tree.nodes if n.type=='BSDF_PRINCIPLED'),None) if mat and mat.node_tree else None
  base=tuple(p.inputs['Base Color'].default_value[:3]) if p else (.4,.4,.4)
  rough=float(p.inputs['Roughness'].default_value) if p else .5
  metal=float(p.inputs['Metallic'].default_value) if p else .0
  e=tuple(p.inputs['Emission Color'].default_value[:3]) if p else (0.,0.,0.)
  strength=float(p.inputs['Emission Strength'].default_value) if p else 0.
  # La luminosidad del reactor se adapta al rango HDR del juego.
  e=tuple(c*min(strength,4.0) for c in e)
  image=None
  if p and p.inputs['Base Color'].is_linked:
   def image_upstream(node,seen):
    if node in seen:return None
    seen.add(node)
    if node.type=='TEX_IMAGE':return node.image
    for slot in node.inputs:
     for link in slot.links:
      value=image_upstream(link.from_node,seen)
      if value:return value
   image=image_upstream(p.inputs['Base Color'].links[0].from_node,set())
  filename='-'
  if image:
   if image.name not in saved:
    filename=f'ship-{len(saved):02d}.rgb'
    width,height=image.size
    a=np.array(image.pixels[:],dtype=np.float32).reshape(height,width,4)[::-1,:,:3]
    srgb=np.where(a<=.0031308,a*12.92,1.055*np.maximum(a,0.)**(1/2.4)-.055)
    with open(dest/filename,'wb') as f:
     f.write(np.array([width,height],dtype='<u4').tobytes());f.write(np.clip(srgb*255+.5,0,255).astype(np.uint8).tobytes())
    saved[image.name]=filename
   filename=saved[image.name]
  rows.append(' '.join(map(str,(*base,rough,metal,*e,filename))))
 (dest/'ship-materials.txt').write_text('\n'.join(rows)+'\n',encoding='utf-8')
 print('MATERIAL_TEXTURES',len(saved),len(rows))
