import bpy, json
from pathlib import Path
from mathutils import Matrix
root=Path(__file__).resolve().parents[1]
dest=root/'assets'
src=dest/'source/OCASO-1-triangulada.blend'
bpy.ops.wm.open_mainfile(filepath=str(src))
# Las dos láminas de efectos transparentes del Blender no son estructura:
# el renderer opaco las sustituye por luz y partículas, no por discos sólidos.
objects=[o for o in bpy.context.scene.objects if o.type=='MESH' and not o.hide_render
         and not o.name.startswith(('Campo azul anular','HUD holografico'))]
# Copia independiente: no cambia el .blend original ni sus materiales.
deps=bpy.context.evaluated_depsgraph_get()
meshes=[]
for o in objects:
    mesh=bpy.data.meshes.new_from_object(o.evaluated_get(deps),depsgraph=deps)
    obj=bpy.data.objects.new('game_'+o.name,mesh)
    bpy.context.collection.objects.link(obj)
    obj.matrix_world=o.matrix_world.copy()
    meshes.append(obj)
bpy.ops.object.select_all(action='DESELECT')
for o in meshes: o.select_set(True)
bpy.context.view_layer.objects.active=meshes[0]
bpy.ops.object.join()
obj=bpy.context.object
bpy.ops.object.transform_apply(location=True,rotation=True,scale=True)
dec=obj.modifiers.new('CPU silhouette','DECIMATE')
dec.ratio=0.42
bpy.ops.object.modifier_apply(modifier=dec.name)
obj.data.calc_loop_triangles()
# Blender (X,-Y,Z) -> juego (X,Y,-Z), rotacion propia sin reflejar caras.
transform=Matrix(((-1,0,0),(0,0,1),(0,1,0)))
mesh=obj.data
def write_mesh(mesh,name):
    mesh.calc_loop_triangles()
    lines=['# OCASO-1 runtime mesh. Source: supplied triangulated Blender.','mtllib OCASO-1.mtl']
    for v in mesh.vertices:
        p=transform@v.co
        lines.append(f'v {p.x:.6f} {p.y:.6f} {p.z:.6f}')
    normals={}
    loop_normals=[]
    for loop in mesh.loops:
        n=transform@mesh.corner_normals[loop.index].vector
        # Compartir indices de normales iguales evita repetir cada esquina
        # como un vertice distinto en el lector OBJ.
        key=tuple(round(float(c),5) for c in n)
        if key not in normals:
            normals[key]=len(normals)+1
            lines.append(f'vn {key[0]:.5f} {key[1]:.5f} {key[2]:.5f}')
        loop_normals.append(normals[key])
    uv=mesh.uv_layers.active
    for loop in mesh.loops:
        t=uv.data[loop.index].uv if uv else (0.0,0.0)
        lines.append(f'vt {t[0]:.7f} {t[1]:.7f}')
    material=-1
    for tri in mesh.loop_triangles:
        if tri.material_index != material:
            material=tri.material_index
            lines.append(f'usemtl mat_{material}')
        lines.append('f '+' '.join(f'{v+1}/{l+1}/{loop_normals[l]}' for v,l in zip(tri.vertices,tri.loops)))
    (dest/name).write_text('\n'.join(lines)+'\n',encoding='utf-8')
    print('LOD',name,len(mesh.vertices),len(mesh.loop_triangles))
write_mesh(mesh,'OCASO-1.obj')
for name,ratio in [('OCASO-1-medium.obj',0.32),('OCASO-1-low.obj',0.08)]:
    lod=bpy.data.objects.new(name,mesh.copy())
    bpy.context.collection.objects.link(lod)
    bpy.context.view_layer.objects.active=lod
    dec=lod.modifiers.new('Distance LOD','DECIMATE')
    dec.ratio=ratio
    bpy.ops.object.modifier_apply(modifier=dec.name)
    write_mesh(lod.data,name)
mtl=[]
for i,m in enumerate(mesh.materials):
    rgb=list(m.diffuse_color[:3]) if m else [.4,.4,.4]
    name=m.name if m else ''
    emission=any(k in name for k in ['LED','Plasma azul','Nucleo de plasma','Pantallas','Instrumentos','HUD de'])
    rgb=[100.0+i,0.0,0.0]
    mtl += [f'# {name}',f'newmtl mat_{i}',f'Kd {rgb[0]:.5f} {rgb[1]:.5f} {rgb[2]:.5f}','']
(dest/'OCASO-1.mtl').write_text('\n'.join(mtl),encoding='utf-8')
print('EXPORT',json.dumps({'vertices':len(mesh.vertices),'triangles':len(mesh.loop_triangles),'materials':len(mesh.materials),'forward':'-Z','up':'+Y'}))

# Exportar también los materiales y las ocho imágenes empacadas del Blender.
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent))
from export_materials import export_materials
export_materials(mesh,dest/'textures')
