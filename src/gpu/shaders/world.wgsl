// Scene-space lighting and simulation. All coordinates are shared wall
// coordinates; cropping happens only in the terminal packing pass.
struct Uniforms {
    dims: vec4<u32>, ids: vec4<u32>, time: vec4<f32>,
    primary: vec4<f32>, secondary: vec4<f32>, background: vec4<f32>,
}
@group(0) @binding(0) var<storage, read> state: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> next: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> pixels: array<u32>;
@group(0) @binding(3) var<uniform> U: Uniforms;
const PI: f32 = 3.14159265;
const TAU: f32 = 6.2831853;
override SCENE: u32 = 0u;
fn hash(n: u32) -> u32 { var v = n ^ U.ids.z; v = (v ^ (v >> 16u)) * 0x7feb352du; v = (v ^ (v >> 15u)) * 0x846ca68bu; return v ^ (v >> 16u); }
fn rnd(n: u32) -> f32 { return f32(hash(n) & 0xffffffu) / 16777216.0; }
fn h2(p: vec2<i32>) -> f32 { return rnd(bitcast<u32>(p.x) * 1597334677u ^ bitcast<u32>(p.y) * 3812015801u); }
fn noise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p)); let f = fract(p); let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(h2(i), h2(i + vec2(1,0)), u.x), mix(h2(i + vec2(0,1)), h2(i + vec2(1,1)), u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
    var q = p; var v = 0.0; var a = 0.5;
    for (var i = 0; i < 4; i++) { v += a * noise(q); q = vec2(q.x * 1.6 - q.y * 1.2, q.x * 1.2 + q.y * 1.6) + 7.3; a *= 0.5; }
    return v;
}
fn aa() -> f32 { return 1.0 / f32(max(U.dims.y, 1u)); }
fn ink(d: f32) -> f32 { return 1.0 - smoothstep(-aa(), aa(), d); }
fn stroke(d: f32, width: f32) -> f32 { return 1.0 - smoothstep(width, width + aa(), abs(d)); }
fn line(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let v = b - a; return length(p - a - v * clamp(dot(p-a,v) / max(dot(v,v), 0.000001), 0.0, 1.0));
}
fn box(p: vec2<f32>, halfsize: vec2<f32>) -> f32 { let q = abs(p)-halfsize; return length(max(q,vec2(0.0)))+min(max(q.x,q.y),0.0); }
fn rotate(p: vec2<f32>, a: f32) -> vec2<f32> { return vec2(cos(a)*p.x-sin(a)*p.y, sin(a)*p.x+cos(a)*p.y); }
fn primary() -> vec3<f32> { return U.primary.rgb; }
fn secondary() -> vec3<f32> { return U.secondary.rgb; }
fn background() -> vec3<f32> { return U.background.rgb; }
fn over(c: vec3<f32>, paint: vec3<f32>, a: f32) -> vec3<f32> { return mix(c,paint,clamp(a,0.0,1.0)); }
fn glow(d: f32, r: f32) -> f32 { return exp(-max(d,0.0)*max(d,0.0)/max(r*r,0.0000001)); }
fn light_sphere(q: vec2<f32>, r: f32, tint: vec3<f32>, metal: f32) -> vec3<f32> {
    // Masked-out pixels still evaluate mix's operands. Avoid out-of-sphere
    // specular overflow (0 * infinity becomes NaN and blackens the background).
    if dot(q,q)>(r+aa())*(r+aa()) {return vec3(0.0);}
    let xy = q/r; let z = sqrt(max(0.0,1.0-dot(xy,xy)));
    let n = vec3(xy,z); let l = normalize(vec3(-0.45,-0.65,1.0));
    let diffuse = max(dot(n,l),0.0); let spec = pow(clamp(dot(n,normalize(l+vec3(0.0,0.0,1.0))),0.0,1.0), mix(18.0,90.0,metal));
    return tint*(0.12+0.72*diffuse)+mix(vec3(0.8),tint,metal)*spec*0.8;
}
fn stars(uv: vec2<f32>, t: f32) -> vec3<f32> {
    let cell = floor(uv*vec2(95.0*U.time.w,95.0)); let h = h2(vec2<i32>(cell));
    let p = fract(uv*vec2(95.0*U.time.w,95.0))-vec2(0.2+0.6*h,0.2+0.6*fract(h*13.1));
    return secondary()*pow(max(0.0,1.0-length(p)*4.0),3.0)*step(0.955,h)*(0.7+0.3*sin(t*(0.5+h)+h*90.0));
}
fn atmosphere(uv: vec2<f32>, t: f32) -> vec3<f32> {
    let sun = vec2(0.73,0.28); let d = length((uv-sun)*vec2(U.time.w,1.0));
    var c = mix(background(), primary()*0.7, pow(clamp(uv.y,0.0,1.0),0.6));
    c += secondary()*(glow(d,0.11)*0.4+glow(d,0.017)*1.1);
    let f = fbm(vec2(uv.x*4.0*U.time.w+t*0.018,uv.y*6.0));
    let body = smoothstep(0.47,0.76,f); let shadow = fbm(vec2(uv.x*4.0*U.time.w+t*0.018,uv.y*6.0+0.08));
    return over(c, mix(primary()*0.42,secondary(),clamp((f-shadow)*9.0+0.65,0.0,1.0)),body*0.85);
}
fn water(uv: vec2<f32>, t: f32) -> vec3<f32> {
    let y = max(uv.y-0.32,0.002); let world = vec2((uv.x-0.5)*U.time.w, 0.18/y);
    let w = sin(world.x*24.0+t*0.9+sin(world.y*2.0))*0.5+sin(world.x*43.0-world.y*8.0+t*1.7)*0.25;
    let normal = vec3(cos(world.x*24.0+t)*0.25,0.8,w*0.3);
    let glint = pow(max(0.0,1.0-abs(uv.x-0.68+w*0.012)/(0.018+y*0.2)),5.0);
    let ripple = pow(max(0.0,sin(world.y*18.0+w+t)),12.0);
    let fresnel = pow(1.0-clamp(y,0.0,1.0),3.0);
    return mix(background(),primary()*0.46,fresnel)+primary()*w*0.045+secondary()*glint*ripple*(0.35+y);
}
fn fish(p: vec2<f32>, center: vec2<f32>, r: f32, angle: f32, phase: f32, tint: vec3<f32>) -> vec4<f32> {
    let q = rotate(p-center,angle); let body = length(q/vec2(r*2.2,r));
    let tailp = q+vec2(r*2.1,0.0); let tail = max(abs(tailp.y+sin(phase)*r*0.3)-max(0.0,-tailp.x)*0.7, max(tailp.x,-tailp.x-r*1.4));
    let mask = max(ink((body-1.0)*r), ink(tail)*0.65);
    var c = tint*(0.35+0.6*sqrt(max(0.0,1.0-pow(q.y/r,2.0))));
    c += secondary()*glow(length(q-vec2(r*0.3,-r*0.3)),r*0.35)*0.35;
    c = over(c,vec3(0.012),ink(length(q-vec2(r*1.2,-r*0.1))-r*0.1));
    return vec4(c,mask);
}
fn underwater(uv: vec2<f32>, t: f32, tank: bool) -> vec3<f32> {
    let p = (uv-0.5)*vec2(U.time.w,1.0);
    var c = mix(primary()*0.21,background()*0.45,uv.y);
    let rays = pow(max(0.0,sin((uv.x+uv.y*0.24)*39.0+t*0.18)),8.0);
    c += secondary()*rays*0.07*exp(-uv.y*2.0);
    let caustic = abs(sin(uv.x*37.0+sin(uv.y*19.0+t))*sin(uv.y*23.0+sin(uv.x*21.0-t*0.7)));
    c += primary()*pow(1.0-caustic,18.0)*0.07*uv.y;
    let count = u32(12.0*U.time.z);
    for (var i=0u;i<count;i++) {
        let z = 0.25+rnd(i+11u)*0.75; let x = fract(rnd(i+31u)+t*(0.008+z*0.012));
        let pos = vec2((x-0.5)*U.time.w, (rnd(i+71u)-0.5)*0.74+sin(t*0.3+f32(i))*0.025);
        let f = fish(p,pos,0.009+0.012*z,sin(t*0.23+f32(i))*0.12,t*3.0+f32(i),mix(primary(),secondary(),rnd(i+91u)));
        c = over(c,f.rgb*z,f.a);
    }
    if tank {
        for (var i=0u;i<18u;i++) {
            let x = rnd(i+233u); let height = 0.1+rnd(i+255u)*0.25;
            let tip = vec2((x-0.5)*U.time.w+sin(t*0.6+f32(i))*0.025,0.5-height);
            c = over(c,primary()*0.45,stroke(line(p,vec2((x-0.5)*U.time.w,0.5),tip),0.004));
        }
        c = over(c,secondary()*0.18,ink(0.47-p.y+noise(vec2(p.x*35.0,0.0))*0.02));
    }
    return c;
}
fn rainfield(uv: vec2<f32>, t: f32, base: vec3<f32>) -> vec3<f32> {
    var c = base;
    for (var layer=0u;layer<3u;layer++) {
        let z = f32(layer)+1.0; let p = uv*vec2(22.0*U.time.w/z,13.0/z)+vec2(t*0.08*z,t*0.65*z);
        let cell = vec2<i32>(floor(p)); let f = fract(p); let h = h2(cell);
        let x = 0.18+0.64*h; let d = length((f-vec2(x,0.3))*vec2(1.0,0.17));
        let body = 1.0-smoothstep(0.011*z,0.026*z,d);
        let shine = glow(length((f-vec2(x-0.012*z,0.27))*vec2(1.0,0.17)),0.012*z);
        c = over(c, mix(base*0.7,secondary(),0.32+0.5*shine),body*step(0.52,h));
    }
    return c;
}
fn skyline(uv: vec2<f32>, t: f32) -> vec3<f32> {
    var c = background()+stars(uv,t)*0.5;
    for (var layer=0;layer<3;layer++) {
        let z = f32(layer); let x = uv.x*(19.0+z*7.0)*U.time.w; let ix = i32(floor(x));
        let top = 0.25+z*0.15+h2(vec2(ix,layer))*0.24;
        if uv.y>top && uv.y<0.85 {
            c = mix(primary()*0.15,background()*0.35,z/3.0);
            let grid = vec2(fract(x*4.0),fract(uv.y*80.0));
            let on = h2(vec2<i32>(floor(vec2(x*4.0,uv.y*80.0))));
            let lit = step(0.7,on)*step(0.25,grid.x)*step(grid.x,0.66)*step(0.2,grid.y)*step(grid.y,0.55);
            c += mix(primary(),secondary(),on)*lit*(0.35+z*0.15);
        }
    }
    if uv.y>0.85 {
        c = background()*0.7;
        c += secondary()*pow(max(0.0,sin(uv.x*91.0+sin(uv.y*200.0+t)*0.07)),18.0)*0.09;
        for (var i=0u;i<7u;i++) {
            let x = fract(rnd(i)+t*(0.025+rnd(i+21u)*0.02));
            c += mix(primary(),secondary(),rnd(i+31u))*glow(length((uv-vec2(x,0.9+rnd(i+41u)*0.06))*vec2(U.time.w,2.0)),0.005)*0.9;
        }
    }
    return c;
}
fn mountains(uv: vec2<f32>, t: f32) -> vec3<f32> {
    var c = background();
    let paper = noise(uv*vec2<f32>(U.dims.xy)*0.5)*0.025;
    c *= 0.98+paper;
    for (var layer=0;layer<7;layer++) {
        let z=f32(layer); let y = fract(uv.y+t*0.012+z*0.157);
        let ridge = 0.33+fbm(vec2(uv.x*(2.0+z*0.4)*U.time.w+z*11.0,z))*0.4;
        let density = ink((ridge-y)*0.4)*exp(-max(0.0,y-ridge)*8.0)*(0.12+z*0.065);
        let texture = 0.7+0.3*fbm(vec2(uv.x*60.0,y*8.0)+z*9.0);
        c = over(c,primary()*texture,density);
        if layer>3 {
            // Sparse ridge pines anchored to the painted terrain, with the
            // same world-space pan as the mountains rather than screen drift.
            let treecell=floor(uv.x*12.0);let tx=(treecell+0.3+h2(vec2<i32>(i32(treecell),layer))*0.4)/12.0;
            let top=0.33+fbm(vec2(tx*(2.0+z*0.4)*U.time.w+z*11.0,z))*0.4;
            let height=0.018+0.028*h2(vec2<i32>(layer,i32(treecell)));
            let q=vec2((uv.x-tx)*U.time.w,y-top);
            let trunk=stroke(q.x,0.0008)*step(-height,q.y)*step(q.y,0.005);
            let crown=ink(max(abs(q.x)-(q.y+height)*0.32,max(-q.y-height,q.y+height*0.15)));
            let branches=0.6+0.4*step(0.3,fract((q.y+height)*240.0));
            c=over(c,primary()*0.85,max(trunk,crown*branches)*(0.3+z*0.07));
            // A narrow waterfall dissolves into the lower wash.
            let fx=0.2+0.6*rnd(u32(layer)+170u);
            let fallridge=0.33+fbm(vec2(fx*(2.0+z*0.4)*U.time.w+z*11.0,z))*0.4;
            let fall=stroke((uv.x-fx)*U.time.w+sin(y*38.0)*0.0015,0.001);
            let fallmask=step(fallridge+0.018,y)*(1.0-smoothstep(fallridge+0.07,fallridge+0.2,y));
            c=over(c,background(),fall*fallmask*0.65);
        }
    }
    // Tiny distant birds, with feather-width wings rather than glowing dots.
    for(var i=0u;i<5u;i++) {
        let q=(uv-vec2(fract(t*0.006+0.18+f32(i)*0.03),0.21+sin(f32(i))*0.01))*vec2(U.time.w,1.0);
        let wing=sin(t*2.0+f32(i))*0.002;
        let d=min(line(q,vec2(-0.004,-wing),vec2(0.0)),line(q,vec2(0.0),vec2(0.004,-wing)));
        c=over(c,primary(),stroke(d,0.0005)*0.6);
    }
    if U.ids.y>0u {c+=vec3(0.9,0.88,0.8)*glow(length((uv-vec2(0.76,0.11))*vec2(U.time.w,1.0)),0.019)*0.55;}
    // Fine red seal, held in the lower corner of the scroll.
    let seal=box((uv-vec2(0.91,0.92))*vec2(U.time.w,1.0),vec2(0.016));
    c=over(c,vec3(0.58,0.16,0.11),stroke(seal,0.0015));
    return c;
}
fn forest(uv: vec2<f32>, t: f32, grass: bool) -> vec3<f32> {
    var c=background()+stars(uv,t)*0.4; let p=(uv-0.5)*vec2(U.time.w,1.0);
    for(var i=0u;i<70u;i++) {
        let x=(rnd(i+11u)-0.5)*U.time.w; let z=0.25+0.75*rnd(i+22u);
        let h=select(0.3+z*0.65,0.05+z*0.33,grass); let base=vec2(x,0.52);
        let bend=sin(t*0.6+x*4.0)*0.04*z; let tip=base+vec2(bend,-h);
        c=over(c,primary()*(0.2+0.5*z),stroke(line(p,base,tip),select(0.004,0.0015,grass)*z));
        if !grass {
            for(var j=0;j<5;j++) {
                let k=f32(j)/5.0; let q=mix(base,tip,0.3+k*0.65);
                let branch=q+vec2(select(-1.0,1.0,j%2==0)*(0.03+0.07*z)*(1.0-k),-0.03);
                c=over(c,primary()*(0.22+0.55*z),stroke(line(p,q,branch),0.002*z));
                c=over(c,primary()*(0.3+0.4*z),ink(length((p-branch)/vec2(1.5,0.6))-0.017*z)*0.8);
            }
        } else {
            c+=secondary()*glow(length(p-tip),0.0018)*0.6;
        }
    }
    return c;
}
fn particle_field(uv: vec2<f32>, t: f32, mode: u32) -> vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0); var c=background()+stars(uv,t)*0.3;
    let count=u32(28.0*U.time.z);
    for(var i=0u;i<count;i++) {
        let z=0.25+rnd(i+19u)*0.75;
        var pos=vec2((rnd(i+2u)-0.5)*U.time.w,(rnd(i+32u)-0.5));
        var r=0.003*z;
        if mode==1u { pos.y=0.55-fract(rnd(i+32u)+t*0.025*z)*1.1; pos.x+=sin(t*0.25+f32(i))*0.025; r=0.015*z; }
        else if mode==2u { pos+=vec2(sin(t*z+f32(i)),cos(t*z*0.7+f32(i)))*0.025; }
        else { let depth=0.1+fract(rnd(i+31u)-t*0.12); pos/=depth; r=0.0015/depth; }
        let d=length(p-pos); let tw=0.45+0.55*pow(0.5+0.5*sin(t*(0.7+z)+f32(i)),3.0);
        if mode==1u {
            let shape=box(p-pos,vec2(r*0.7,r));
            c=over(c,primary()*(0.35+0.5*glow(d,r*0.65)),ink(shape));
            c+=secondary()*glow(d,r*0.55)*0.55;
        } else { c+=mix(primary(),secondary(),z)*tw*(glow(d,r)*0.8+glow(d,r*4.0)*0.06); }
    }
    return c;
}
fn fireworks(uv: vec2<f32>, t: f32) -> vec3<f32> {
    var c=background()*0.7; let p=(uv-0.5)*vec2(U.time.w,1.0);
    for(var burst=0u;burst<4u;burst++) {
        let clock=t*0.25+f32(burst)*0.29; let life=fract(clock); let cycle=u32(floor(clock))+burst*997u;
        let center=vec2((rnd(cycle+21u)-0.5)*U.time.w*0.7,-0.1+(rnd(cycle+31u)-0.5)*0.4);
        for(var i=0u;i<36u;i++) {
            let a=f32(i)*TAU/36.0; let velocity=vec2(cos(a),sin(a))*(0.22+0.05*rnd(i+cycle));
            let at=center+velocity*(1.0-exp(-life*3.0))+vec2(0.0,life*life*0.2);
            let tail=at-velocity*0.05;
            let tint=mix(primary(),secondary(),rnd(cycle));
            c+=tint*(glow(line(p,at,tail),0.0018)+glow(length(p-at),0.008)*0.09)*pow(1.0-life,1.5);
        }
    }
    return c;
}
fn smoke(uv: vec2<f32>, t: f32, liquid: bool) -> vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0); let y=1.0-uv.y;
    let bend=sin(y*8.0-t*0.6)*0.03+sin(y*19.0+t*0.4)*0.015;
    let width=0.005+y*y*select(0.12,0.35,liquid);
    let density=exp(-pow((p.x-bend)/width,2.0))*smoothstep(0.0,0.15,y)*(1.0-smoothstep(0.6,1.0,y));
    let turbulent=fbm(vec2(p.x*18.0,y*8.0-t*0.3));
    var c=background()+primary()*density*(0.2+turbulent*0.65);
    if !liquid { c+=secondary()*glow(length(p-vec2(0.0,0.4)),0.012)*0.7; }
    return c;
}
fn aurora(uv: vec2<f32>,t: f32) -> vec3<f32> {
    var c=background()+stars(uv,t); let y=select(uv.y,1.6-uv.y,uv.y>0.8);
    for(var i=0;i<3;i++) {
        let f=f32(i); let x=uv.x*U.time.w;
        let ridge=0.26+0.08*f+sin(x*3.0+t*0.13+f)*0.09+sin(x*7.0-t*0.09)*0.02;
        let curtain=exp(-max(0.0,ridge-y)*14.0)*smoothstep(ridge+0.02,ridge,y);
        let ray=0.5+0.5*pow(0.5+0.5*sin(x*160.0+sin(x*11.0+t)*4.0),3.0);
        c+=mix(primary(),secondary(),f*0.3)*curtain*ray*0.27*select(1.0,0.4,uv.y>0.8);
    }
    return c;
}
fn orbits(uv: vec2<f32>,t: f32) -> vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0); var c=background()+stars(uv,t);
    c+=secondary()*(glow(length(p),0.035)*1.3+glow(length(p),0.1)*0.1);
    for(var i=1;i<7;i++) {
        let r=0.06+f32(i)*0.045; let a=t/(0.5+f32(i)*0.75)+f32(i);
        let pos=vec2(cos(a),sin(a)*0.6)*r;
        c+=primary()*stroke(length(p/vec2(1.0,0.6))-r,0.0007)*0.16;
        let pr=0.006+rnd(u32(i))*0.012;
        c=over(c,light_sphere(p-pos,pr,mix(primary(),secondary(),rnd(u32(i+50))),0.2),ink(length(p-pos)-pr));
    }
    return c;
}
fn mechanical(uv: vec2<f32>,t: f32,mode: u32) -> vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0); var c=background();
    if mode==0u {
        for(var i=0;i<7;i++) {
            let pos=vec2((f32(i%3)-1.0)*0.2,(f32(i/3)-1.0)*0.2);
            let q=p-pos; let angle=atan2(q.y,q.x)+t*select(-0.3,0.3,i%2==0);
            let r=0.092+0.009*step(0.1,sin(angle*16.0));
            let d=max(length(q)-r,0.035-length(q));
            let metal=primary()*(0.3+0.45*(0.5+0.5*cos(angle-0.8)))+secondary()*stroke(d,0.0015)*0.4;
            c=over(c,metal,ink(d));
            c=over(c,secondary()*0.7,ink(length(q)-0.011));
            for(var j=0;j<4;j++) {
                let a=f32(j)*PI*0.5-t*select(-0.3,0.3,i%2==0);
                c=over(c,metal,stroke(line(q,vec2(cos(a),sin(a))*0.014,vec2(cos(a),sin(a))*0.07),0.008));
            }
        }
    } else {
        for(var i=0;i<12;i++) {
            let x=(f32(i)/11.0-0.5)*U.time.w*0.7;
            let angle=sin(t*(1.0+f32(i)*0.035))*0.55;
            let anchor=vec2(x,-0.35); let bob=anchor+vec2(sin(angle),cos(angle))*0.5;
            c=over(c,secondary()*0.35,stroke(line(p,anchor,bob),0.001));
            c=over(c,light_sphere(p-bob,0.018,primary(),0.8),ink(length(p-bob)-0.018));
        }
    }
    return c;
}
fn bubbles(uv: vec2<f32>,t: f32,lava: bool) -> vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0); var c=background();
    if lava {
        // Implicit metaball surface: blobs really merge instead of overlapping
        // like solid marbles. The field gradient supplies the surface normal.
        var field=0.0;var gradient=vec2(0.0);
        for(var i=0u;i<9u;i++) {
            let phase=rnd(i+20u)*TAU;
            let radius=0.045+rnd(i+10u)*0.05;
            let pos=vec2(sin(t*0.13+phase)*U.time.w*0.32,cos(t*0.11+phase*1.7)*0.43);
            let q=p-pos;let d2=max(dot(q,q),0.0002);let r2=radius*radius;
            field+=r2/d2;gradient-=2.0*q*r2/(d2*d2);
        }
        let mask=smoothstep(0.98,1.06,field);
        let normal=normalize(vec3(-gradient*0.025,1.0));
        let lighting=0.3+0.7*max(dot(normal,normalize(vec3(-0.5,-0.65,1.0))),0.0);
        let tint=mix(primary()*0.35,secondary(),smoothstep(1.0,3.5,field));
        c+=primary()*smoothstep(0.45,1.0,field)*0.075;
        return over(c,tint*lighting+secondary()*pow(max(normal.z,0.0),16.0)*0.08,mask);
    }
    for(var i=0u;i<12u;i++) {
        let r=0.025+rnd(i+10u)*0.09;
        let pos=vec2((rnd(i)-0.5)*U.time.w*0.85,0.6-fract(rnd(i+20u)+t*0.018)*1.2);
        let q=p-pos+vec2(sin(t*0.3+f32(i))*0.03,0.0); let d=length(q);
        let tint=mix(primary(),secondary(),rnd(i+30u));
        c=over(c,light_sphere(q,r,tint,select(0.65,0.05,lava)),ink(d-r));
        if lava { c+=tint*glow(d,r*1.2)*0.1; }
    }
    return c;
}
fn ribbon(uv: vec2<f32>,t: f32) -> vec3<f32> {
    var c=background();
    for(var i=0;i<5;i++) {
        let f=f32(i); let ridge=0.25+f*0.12+sin(uv.x*7.0*U.time.w+t*0.4+f)*0.07;
        let d=uv.y-ridge; let width=0.018+0.01*sin(uv.x*4.0+t+f);
        let light=0.3+0.5*sqrt(max(0.0,1.0-pow(d/width,2.0)));
        c=over(c,mix(primary(),secondary(),f/5.0)*light,ink(abs(d)-width));
    }
    return c;
}
fn abstract_field(uv: vec2<f32>,t: f32,mode:u32) -> vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0); var value=0.0;
    if mode==0u { value=0.5+0.5*sin(p.x*9.0+t+sin(p.y*7.0-t*0.7)+sin(length(p)*13.0-t*0.5)); }
    if mode==1u { let r=max(length(p),0.003); value=0.5+0.5*sin(0.8/r-t*2.0+atan2(p.y,p.x)*6.0); value*=smoothstep(0.01,0.2,r); }
    if mode==2u { value=pow(0.5+0.5*sin(fbm(p*4.0+vec2(t*0.04,0.0))*45.0),12.0); }
    var c=mix(background(),mix(primary(),secondary(),value),value*0.85);
    c+=secondary()*pow(value,24.0)*0.15;
    return c;
}
fn mosaic(uv:vec2<f32>,t:f32)->vec3<f32> {
    let p=uv*vec2(U.time.w,1.0)*13.0; let cell=vec2<i32>(floor(p));
    var d1=10.0;var d2=10.0;var hue=0.0;
    for(var y=-1;y<=1;y++) {for(var x=-1;x<=1;x++) {
        let at=cell+vec2(x,y);let h=h2(at);let center=vec2<f32>(at)+vec2(0.2+0.6*h,0.2+0.6*fract(h*7.9));
        let d=length(p-center);
        if d<d1 {d2=d1;d1=d;hue=h;}else{d2=min(d2,d);}
    }}
    let light=0.6+0.3*sin(t*0.35+hue*TAU);
    return mix(primary(),secondary(),hue)*light*smoothstep(0.025,0.07,d2-d1)+secondary()*stroke(d2-d1-0.055,0.008)*0.18;
}
fn sonar(uv:vec2<f32>,t:f32)->vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0);let r=length(p);let angle=atan2(p.y,p.x);
    let sweep=fract((t*0.7-angle)/TAU);var c=background();
    c+=primary()*exp(-sweep*18.0)*0.25*step(r,0.45);
    c+=primary()*stroke(fract(r*10.0)*0.1,0.001)*0.25;
    c+=primary()*(stroke(p.x,0.001)+stroke(p.y,0.001))*0.2;
    for(var i=0u;i<14u;i++) {let q=(vec2(rnd(i),rnd(i+31u))-0.5)*0.7;c+=secondary()*glow(length(p-q),0.005)*exp(-sweep*3.0);}
    return c;
}
fn road(uv:vec2<f32>,t:f32,aerial:bool)->vec3<f32> {
    var c=background()+stars(uv,t)*select(0.6,0.0,aerial);
    let depth=max(uv.y-0.35,0.002); let x=(uv.x-0.5)*U.time.w;
    let width=select(depth*0.7,0.23,aerial);let inside=ink(abs(x)-width);
    c=over(c,vec3(0.035,0.042,0.053),inside);
    let stripes=step(0.45,fract(select(0.5/depth,uv.y*12.0,aerial)-t*1.5));
    c+=secondary()*(stroke(x-width*0.92,0.0015)+stroke(x+width*0.92,0.0015))*step(0.35,uv.y)*0.6;
    c+=secondary()*stroke(x,0.0015)*stripes*inside*0.5;
    for(var i=0u;i<12u;i++) {
        let y=fract(rnd(i)+t*0.04);let w=select(max(y-0.35,0.0)*0.7,0.23,aerial);
        let pos=vec2(w*select(-0.5,0.5,i%2u==0u),y);
        c+=mix(vec3(0.9,0.12,0.04),secondary(),f32(i%2u))*glow(length((vec2(x,uv.y)-pos)*vec2(1.0,0.3)),0.004)*step(0.35,y);
    }
    return c;
}
fn graph(uv:vec2<f32>,t:f32,circuit:bool)->vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0);var c=background();
    for(var i=0u;i<22u;i++) {
        var a=(vec2(rnd(i*2u),rnd(i*2u+1u))-0.5)*vec2(U.time.w,0.85);
        var b=(vec2(rnd((i+1u)*2u),rnd((i+1u)*2u+1u))-0.5)*vec2(U.time.w,0.85);
        if !circuit { a+=vec2(sin(t*0.2+f32(i)),cos(t*0.15+f32(i)))*0.02;b+=vec2(sin(t*0.2+f32(i+1u)),cos(t*0.15+f32(i+1u)))*0.02; }
        let elbow=vec2(a.x,b.y);let d=select(line(p,a,b),min(line(p,a,elbow),line(p,elbow,b)),circuit);
        c+=primary()*stroke(d,0.0012)*0.35;
        c+=secondary()*glow(length(p-a),0.004)*0.7;
        let pulse=mix(a,b,fract(t*0.2+rnd(i+67u)));
        if !circuit {c+=secondary()*glow(length(p-pulse),0.003)*0.6;}
    }
    return c;
}
fn curve(uv:vec2<f32>,t:f32)->vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0);var c=background();
    var prev=vec2(0.0);
    for(var i=0;i<128;i++) {
        let a=f32(i)*0.075+t*0.08;let at=vec2(sin(a*3.0)*0.3+sin(a*3.01+t*0.05)*0.08,cos(a*2.0)*0.3);
        if i>0 {c+=mix(primary(),secondary(),f32(i)/128.0)*glow(line(p,prev,at),0.0018)*0.35;}
        prev=at;
    }
    return c;
}
fn grid(uv:vec2<f32>,t:f32)->vec3<f32> {
    var c=background();let y=uv.y-0.48;
    if y>0.0 {
        let q=vec2((uv.x-0.5)*U.time.w/y,0.25/y-t*0.5);
        let g=min(abs(fract(q.x+0.5)-0.5),abs(fract(q.y+0.5)-0.5));
        c+=primary()*exp(-g*80.0)*min(1.0,y*4.0);
    }else{
        let d=length((uv-vec2(0.5,0.32))*vec2(U.time.w,1.0));
        c+=secondary()*ink(d-0.11)*(0.5+0.5*step(0.28,fract(uv.y*90.0)));
    }
    return c;
}
fn frost(uv:vec2<f32>,t:f32)->vec3<f32> {
    let p=(uv-0.5)*vec2(U.time.w,1.0);var c=background();let growth=clamp(t*0.035,0.05,1.0);
    for(var i=0u;i<7u;i++) {
        let center=(vec2(rnd(i+71u),rnd(i+93u))-0.5)*vec2(U.time.w,0.8);
        let q=p-center;let angle=atan2(q.y,q.x);let radial=length(q);
        let sector=abs(sin(angle*3.0));let arm=radial*sector;
        let branch=abs(fract(radial*52.0-sector*2.0)-0.5)*0.013;
        let shape=min(arm,branch+arm*0.12);
        let mask=stroke(shape,0.001)*step(radial,(0.1+rnd(i)*0.16)*growth);
        c+=mix(primary(),secondary(),0.5+0.5*cos(angle-t*0.1))*mask*0.65;
    }
    return c;
}
fn sindex(p:vec2<i32>)->u32 { let size=vec2<i32>(U.dims.zw);let q=(p%size+size)%size;return u32(q.y)*U.dims.z+u32(q.x); }
fn getstate(p:vec2<i32>)->vec4<f32>{return state[sindex(p)];}
@compute @workgroup_size(8,8)
fn initialize(@builtin(global_invocation_id) gid:vec3<u32>) {
    if any(gid.xy>=U.dims.zw){return;}let i=gid.y*U.dims.z+gid.x;let h=rnd(i);let uv=vec2<f32>(gid.xy)/vec2<f32>(U.dims.zw);
    var v=vec4(0.0);
    switch U.ids.x {
        case 2u:{v.x=select(0.0,0.8+0.2*h,gid.y+2u>=U.dims.w);}
        case 6u:{v.x=step(0.79,h);v.y=v.x;}
        case 7u:{v=vec4(rnd(i*4u),rnd(i*4u+1u),cos(h*TAU)*0.04,sin(h*TAU)*0.04);}
        case 20u:{v.x=select(0.0,1.0,uv.y>0.87+noise(vec2(uv.x*7.0,0.0))*0.08);}
        case 42u:{let seed=step(0.82,noise(uv*20.0));v=vec4(1.0-seed*0.5,seed*0.3,0.0,0.0);}
        default:{}
    }
    next[i]=v;
}
@compute @workgroup_size(8,8)
fn simulate(@builtin(global_invocation_id) gid:vec3<u32>) {
    if any(gid.xy>=U.dims.zw){return;}let p=vec2<i32>(gid.xy);let i=gid.y*U.dims.z+gid.x;var v=state[i];
    switch U.ids.x {
        case 2u:{
            let drift=i32(rnd(i+U.ids.w*997u)*3.0)-1;
            let heat=(getstate(p+vec2(drift,1)).x*2.0+getstate(p+vec2(-1,1)).x+getstate(p+vec2(1,1)).x)*0.25;
            v.x=max(0.0,heat-0.002-rnd(i+U.ids.w)*0.004);
            if gid.y+2u>=U.dims.w{v.x=0.75+rnd(gid.x+U.ids.w*71u)*0.25;}
        }
        case 6u:{
            // Generation cadence is 10Hz, independent of presentation rate.
            if U.ids.w%6u==0u {
                var n=0.0;for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){if x!=0||y!=0{n+=getstate(p+vec2(x,y)).x;}}}
                v.x=select(0.0,1.0,n==3.0||(v.x>0.5&&n==2.0));
            }
            v.y=max(v.x,v.y*0.96);
            if U.ids.w>0u && U.ids.w%1800u==0u {
                v.x=step(0.79,rnd(i+U.ids.w));v.y=v.x;
            }
        }
        case 7u:{
            if i>=128u {next[i]=v;return;}
            var force=vec2(0.0);var center=vec2(0.0);var align=vec2(0.0);var n=0.0;
            for(var j=0u;j<128u;j++) {if i==j{continue;}let other=state[j];var d=other.xy-v.xy;d-=round(d);let distance=length(d);
                if distance<0.12 {center+=d;align+=other.zw;n+=1.0;if distance<0.025{force-=d/(distance*distance+0.0001)*0.0008;}}
            }
            if n>0.0{force+=center/n*0.25+(align/n-v.zw)*0.3;}
            let velocity=v.zw+force*U.time.y;let len=length(velocity);let bounded=velocity/max(len,0.00001)*clamp(len,0.025,0.075);v.z=bounded.x;v.w=bounded.y;
            let position=fract(v.xy+v.zw*U.time.y+1.0);v.x=position.x;v.y=position.y;
        }
        case 20u:{
            // Disjoint 2x2 blocks alternate origin each tick. Every invocation
            // derives the same block result, so diagonal settling has no races
            // or duplicated grains when two sources compete for one vacancy.
            let phase=i32(U.ids.w%2u);let local=(p+phase)%2;let base=p-local;
            if all(base>=vec2(0)) && all(base+1<vec2<i32>(U.dims.zw)) {
                var block=vec4(getstate(base).x,getstate(base+vec2(1,0)).x,getstate(base+vec2(0,1)).x,getstate(base+vec2(1,1)).x);
                if block.z<0.5 && block.x>0.5 {block.z=block.x;block.x=0.0;}
                if block.w<0.5 && block.y>0.5 {block.w=block.y;block.y=0.0;}
                if block.z<0.5 && block.y>0.5 {block.z=block.y;block.y=0.0;}
                if block.w<0.5 && block.x>0.5 {block.w=block.x;block.x=0.0;}
                v.x=block[u32(local.y*2+local.x)];
            }
            let emitter=abs(fract(f32(gid.x)/f32(U.dims.z)*3.0)-0.5)<0.025;
            if gid.y==0u && emitter && rnd(gid.x+U.ids.w*73u)>0.4 {v.x=1.0;}
            if gid.y+1u==U.dims.w{v.x=1.0;}
            if U.ids.w>0u && U.ids.w%3600u==0u {v.x=select(0.0,1.0,gid.y+1u==U.dims.w);}
        }
        case 42u:{
            let lap=(getstate(p+vec2(1,0)).xy+getstate(p-vec2(1,0)).xy+getstate(p+vec2(0,1)).xy+getstate(p-vec2(0,1)).xy)*0.2
                +(getstate(p+vec2(1,1)).xy+getstate(p+vec2(1,-1)).xy+getstate(p+vec2(-1,1)).xy+getstate(p-vec2(1,1)).xy)*0.05-v.xy;
            let reaction=v.x*v.y*v.y;v.x+=lap.x-reaction+0.036*(1.0-v.x);v.y+=0.5*lap.y+reaction-0.098*v.y;
            v=clamp(v,vec4(0.0),vec4(1.0));
        }
        default:{}
    }
    next[i]=v;
}
fn simulated(uv:vec2<f32>,t:f32,id:u32)->vec3<f32> {
    let point=vec2<i32>(clamp(uv,vec2(0.0),vec2(0.9999))*vec2<f32>(U.dims.zw));let v=getstate(point);var c=background();
    if id==2u {let heat=v.x;c+=mix(primary()*heat,secondary(),pow(heat,4.0))*heat*1.4;}
    if id==6u {c+=primary()*v.y*0.7+secondary()*v.x*0.3;}
    if id==20u {let shade=getstate(point-vec2(1,1)).x;c=over(c,primary()*(0.45+0.3*rnd(sindex(point)))+secondary()*(1.0-shade)*0.2,v.x);}
    if id==42u {let dx=getstate(point+vec2(1,0)).y-v.y;let dy=getstate(point+vec2(0,1)).y-v.y;let light=clamp(0.55-dx*5.0-dy*8.0,0.1,1.0);c+=mix(primary(),secondary(),smoothstep(0.15,0.4,v.y))*smoothstep(0.025,0.18,v.y)*light;}
    if id==7u {
        let p=(uv-0.5)*vec2(U.time.w,1.0);
        for(var i=0u;i<128u;i++) {let b=state[i];let q=(b.xy-0.5)*vec2(U.time.w,1.0);let dir=normalize(b.zw);let wing=vec2(-dir.y,dir.x)*(0.003+0.0015*sin(t*8.0+f32(i)));
            let d=min(line(p,q-dir*0.004+wing,q),line(p,q-dir*0.004-wing,q));c+=mix(primary(),secondary(),rnd(i))*stroke(d,0.001);}
    }
    return c;
}
// Compensated complex arithmetic keeps the existing 1e-11 deep zoom usable.
fn dsadd(a:vec2<f32>,b:vec2<f32>)->vec2<f32>{let s=a.x+b.x;let v=s-a.x;let e=(a.x-(s-v))+(b.x-v)+a.y+b.y;let h=s+e;return vec2(h,e-(h-s));}
fn dsmul(a:vec2<f32>,b:vec2<f32>)->vec2<f32>{let p=a.x*b.x;let e=fma(a.x,b.x,-p)+a.x*b.y+a.y*b.x;let h=p+e;return vec2(h,e-(h-p));}
fn mandel(uv:vec2<f32>,t:f32)->vec3<f32>{
    let scale=3.0*exp(-fract(t/90.0)*26.0);let q=(uv-0.5)*vec2(U.time.w,1.0)*scale;
    let cx=dsadd(vec2(-0.7436439,1.847768e-8),vec2(q.x,0.0));let cy=dsadd(vec2(0.13182591,-5.481633e-9),vec2(q.y,0.0));
    var x=vec2(0.0);var y=vec2(0.0);var iter=0u;
    for(var i=0u;i<256u;i++){let xx=dsmul(x,x);let yy=dsmul(y,y);if xx.x+yy.x>4.0{break;}y=dsadd(dsmul(x,y)*2.0,cy);x=dsadd(dsadd(xx,-yy),cx);iter++;}
    if iter==256u{return background()*0.3;}
    let v=0.5+0.5*sin(sqrt(f32(iter))*0.8+t*0.04);return mix(primary(),secondary(),v)*(0.2+0.8*v);
}
fn glyph(code:u32,p:vec2<i32>)->f32 {
    if any(p<vec2(0))||p.x>=5||p.y>=7{return 0.0;}
    var rows:array<u32,7>;
    switch code {
        case 0u:{rows=array<u32,7>(31u,4u,4u,4u,4u,4u,4u);}// T
        case 1u:{rows=array<u32,7>(31u,16u,16u,30u,16u,16u,31u);}// E
        case 2u:{rows=array<u32,7>(30u,17u,17u,30u,20u,18u,17u);}// R
        case 3u:{rows=array<u32,7>(17u,27u,21u,21u,17u,17u,17u);}// M
        case 4u:{rows=array<u32,7>(30u,17u,17u,30u,16u,16u,16u);}// P
        case 5u:{rows=array<u32,7>(14u,17u,17u,31u,17u,17u,17u);}// A
        case 6u:{rows=array<u32,7>(30u,17u,17u,17u,17u,17u,30u);}// D
        default:{rows=array<u32,7>(17u,17u,17u,17u,17u,10u,4u);}// V
    }
    return f32((rows[u32(p.y)]>>u32(4-p.x))&1u);
}
fn title(uv:vec2<f32>,t:f32,dvd:bool)->vec3<f32>{
    var center=vec2(0.5);var letters=array<u32,9>(0u,1u,2u,3u,4u,5u,4u,1u,2u);var count=9u;
    if dvd{center=vec2(0.2+abs(fract(t*0.06)*2.0-1.0)*0.6,0.15+abs(fract(t*0.037)*2.0-1.0)*0.7);letters[0]=6u;letters[1]=7u;letters[2]=6u;count=3u;}
    let scale=min(U.time.w/(f32(count)*7.0),0.012);let q=(uv-center)*vec2(U.time.w,1.0)/scale+vec2(f32(count)*3.0,3.5);
    let index=i32(floor(q.x/6.0));var v=0.0;
    if index>=0&&index<i32(count){v=glyph(letters[u32(index)],vec2<i32>(i32(floor(q.x))%6,i32(floor(q.y))));}
    return background()*0.1+select(vec3(0.86),mix(primary(),secondary(),0.5+0.5*sin(t*0.2)),dvd)*v;
}
fn world(uv:vec2<f32>,t:f32)->vec3<f32>{
    let id=SCENE;
    switch id {
        case 0u:{return rainfield(uv,t,skyline(uv,t)*0.55);}
        case 1u:{return particle_field(uv,t,0u);}
        case 2u,6u,7u,20u,42u:{return simulated(uv,t,id);}
        case 3u:{let p=(uv-0.5)*vec2(U.time.w,1.0);var c=background();for(var i=0u;i<15u;i++){let a=(vec2(rnd(i*2u),rnd(i*2u+1u))-0.5)*vec2(U.time.w,0.8);let b=(vec2(rnd(i*2u+2u),rnd(i*2u+3u))-0.5)*vec2(U.time.w,0.8);let elbow=vec2(a.x,b.y);let d=min(line(p,a,elbow),line(p,elbow,b));let mask=ink(d-0.012);c=over(c,light_sphere(vec2(d,0.0),0.014,mix(primary(),secondary(),rnd(i)),0.65),mask*step(f32(i),modulo(t*0.8,18.0)));}return c;}
        case 4u:{return abstract_field(uv,t,0u);}
        case 5u:{return aurora(uv,t);}
        case 8u:{return bubbles(uv,t,true);}
        case 9u:{return abstract_field(uv,t,1u);}
        case 10u:{return title(uv,t,true);}
        case 11u:{return title(uv,t,false);}
        case 12u:{return forest(vec2(uv.x,1.0-uv.y),t,false);}
        case 13u:{return fireworks(uv,t);}
        case 14u:{if uv.y<0.32{return background()+stars(uv,t)+secondary()*glow(length((uv-vec2(0.68,0.15))*vec2(U.time.w,1.0)),0.027);}return water(uv,t);}
        case 15u:{return graph(uv,t,true);}
        case 16u:{return atmosphere(uv,t);}
        case 17u:{return mandel(uv,t);}
        case 18u:{var c=background()+stars(uv,t);let p=(uv-0.5)*vec2(U.time.w,1.0);for(var i=0u;i<6u;i++){let phase=fract(t*0.12+rnd(i));let head=vec2((rnd(i+9u)-0.5)*U.time.w,-0.55)+vec2(-0.7,0.9)*phase;let d=line(p,head,head+vec2(0.12,-0.15));c+=secondary()*glow(d,0.0015)*sin(phase*PI)+primary()*glow(length(p-head),0.006)*0.3;}return c;}
        case 19u:{let p=(uv-0.5)*vec2(U.time.w,1.0);var c=underwater(uv,t,false);for(var i=0u;i<6u;i++){let q=(vec2(rnd(i+9u),rnd(i+15u))-0.5)*vec2(U.time.w,0.8);let d=length(p-q);let leaf=ink(d-0.035)*step(0.3,abs(atan2(p.y-q.y,p.x-q.x)));c=over(c,primary()*0.45,leaf);}return c;}
        case 21u:{return rainfield(uv,t,skyline(uv,t));}
        case 22u:{return underwater(uv,t,false);}
        case 23u:{let p=(uv-0.5)*vec2(U.time.w,1.0);var c=background()*1.5+secondary()*glow(length(p-vec2(-0.25,0.0)),0.3)*0.18;let screen=box(p-vec2(0.04,0.03),vec2(0.17,0.12));c=over(c,primary()*0.15,ink(screen-0.018));c=over(c,skyline((p-vec2(0.04,0.03))/vec2(0.34,0.24)+0.5,t),ink(screen));c+=secondary()*glow(max(screen,0.0),0.04)*0.025;return c;}
        case 24u:{return road(uv,t,true);}
        case 25u:{return graph(uv,t,false);}
        case 26u:{var c=water(vec2(uv.x,0.4+uv.y*0.6),t)*0.7;let p=(uv-0.5)*vec2(U.time.w,1.0);for(var i=0u;i<12u;i++){let age=fract(t*0.16+rnd(i));let q=(vec2(rnd(i+30u),rnd(i+60u))-0.5)*vec2(U.time.w,1.0);let d=length((p-q)*vec2(1.0,1.4));c+=secondary()*stroke(d-age*0.16,0.001)*(1.0-age)*0.25;}return c;}
        case 27u:{return forest(uv,t,true)*0.5+particle_field(uv,t,2u);}
        case 28u:{return particle_field(uv,t,1u);}
        case 29u:{return smoke(uv,t,false);}
        case 30u:{return frost(uv,t);}
        case 31u:{return orbits(uv,t);}
        case 32u:{return ribbon(uv,t);}
        case 33u:{return sonar(uv,t);}
        case 34u:{return abstract_field(uv,t,2u);}
        case 35u:{return mechanical(uv,t,0u);}
        case 36u:{return grid(uv,t);}
        case 37u:{return smoke(uv,t,true);}
        case 38u:{return mosaic(uv,t);}
        case 39u:{return curve(uv,t);}
        case 40u:{let p=uv*vec2(U.time.w,1.0);let f=fbm(p*4.0+vec2(t*0.01,0.0));let dust=fbm(p*8.0+vec2(0.0,t*0.007));return background()+stars(uv,t)*smoothstep(0.7,0.25,dust)+mix(primary(),secondary(),f)*pow(f,3.0)*0.9*(1.0-dust*0.8);}
        case 41u:{return mechanical(uv,t,1u);}
        case 43u:{return forest(uv,t,true);}
        case 44u:{var c=atmosphere(uv,t);if uv.y>0.77{c=primary()*0.25+secondary()*noise(uv*vec2(45.0,100.0))*0.06;}let p=(uv-0.5)*vec2(U.time.w,1.0);let pos=vec2((fract(t*0.025)-0.5)*U.time.w,-0.13);let q=rotate(p-pos,0.12);let body=min(box(q,vec2(0.02,0.003)),box(q+vec2(0.0,0.002),vec2(0.005,0.022)));c=over(c,secondary()*0.8,ink(body));c+=secondary()*stroke(line(p,pos-vec2(0.2,0.025),pos-vec2(0.02,0.002)),0.0015)*0.35;return c;}
        case 45u:{return underwater(uv,t,true);}
        case 46u:{return road(uv,t,false);}
        case 47u:{return bubbles(uv,t,false);}
        case 48u:{return mountains(uv,t);}
        default:{return background();}
    }
}
fn modulo(x:f32,y:f32)->f32{return x-y*floor(x/y);}
@compute @workgroup_size(8,8)
fn draw(@builtin(global_invocation_id) gid:vec3<u32>) {
    if any(gid.xy>=U.dims.xy){return;}
    let uv=(vec2<f32>(gid.xy)+0.5)/vec2<f32>(U.dims.xy);
    let c=clamp(world(uv,U.time.x),vec3(0.0),vec3(1.0));
    let rgb=vec3<u32>(c*255.0);pixels[gid.y*U.dims.x+gid.x]=rgb.x|(rgb.y<<8u)|(rgb.z<<16u);
}
