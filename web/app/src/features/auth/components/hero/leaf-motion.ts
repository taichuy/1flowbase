interface Leaf {
  x: number;
  y: number;
  z: number; // 3D depth: 0.05 (far) to 1.0 (near)
  vx: number;
  vy: number;
  baseVx: number;
  baseVy: number;
  rotation: number;
  rotationSpeed: number;
  scaleX: number;
  scaleXSpeed: number;
  size: number;
  r: number;
  g: number;
  b: number;
  baseAlpha: number;
  veinR: number;
  veinG: number;
  veinB: number;
  veinAlpha: number;
  flutterPhase: number;
  flutterSpeed: number;
  isSinking: boolean;
  opacity: number;
}

interface Ripple {
  x: number;
  y: number;
  z: number; // 3D depth mapping
  radius: number;
  maxRadius: number;
  alpha: number;
  speed: number;
}

// The document opening and the routed scene run the same simulation and input handlers.
export function startLeafMotion(
  canvas: HTMLCanvasElement,
  container: HTMLElement
) {
  const mouse = { x: -1000, y: -1000, vx: 0, vy: 0 };
  let lastMousePos: { x: number; y: number } | null = null;
  const ctx = canvas.getContext('2d');
  if (!ctx) return undefined;

  let animationFrameId: number;
  const leaves: Leaf[] = [];
  let ripples: Ripple[] = [];
  const leafCount = 75;

  // Resize handler to adjust canvas bounds with retina display support
  const resizeCanvas = () => {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;
    ctx.scale(dpr, dpr);
  };

  resizeCanvas();
  window.addEventListener('resize', resizeCanvas);

  // Fluorescent green theme colors matching login button (#00d084)
  const leafColors = [
    { r: 0, g: 208, b: 132, a: 0.28 },
    { r: 0, g: 208, b: 132, a: 0.2 },
    { r: 52, g: 211, b: 153, a: 0.24 },
    { r: 209, g: 250, b: 229, a: 0.35 }
  ];
  const veinColors = [
    { r: 0, g: 208, b: 132, a: 0.45 },
    { r: 0, g: 208, b: 132, a: 0.35 },
    { r: 52, g: 211, b: 153, a: 0.4 },
    { r: 16, g: 185, b: 129, a: 0.38 }
  ];

  const createLeaf = (initYRandom = false): Leaf => {
    const rect = canvas.getBoundingClientRect();
    const width = rect.width || 800;
    const height = rect.height || 600;

    const idx = Math.floor(Math.random() * leafColors.length);
    const leafColor = leafColors[idx];
    const veinColor = veinColors[idx];

    const z = Math.random() * 0.95 + 0.05; // Depth from 0.05 (far) to 1.0 (near)
    const waterHorizon = height * 0.75;
    const waterY = waterHorizon + z * (height * 0.25);

    return {
      x: Math.random() * width,
      y: initYRandom ? Math.random() * (waterY - 30) : -20 - Math.random() * 50,
      z,
      vx: Math.random() * 0.4 - 0.2,
      vy: Math.random() * 0.8 + 0.4,
      baseVx: Math.random() * 0.2 + 0.05, // Slight drift to the right
      baseVy: Math.random() * 0.55 + 0.35, // Falling speed
      rotation: Math.random() * Math.PI * 2,
      rotationSpeed: (Math.random() * 0.02 - 0.01) * 0.4,
      scaleX: Math.random() * 2 - 1,
      scaleXSpeed: Math.random() * 0.01 + 0.005,
      size: Math.random() * 7 + 5, // 5px to 12px size
      r: leafColor.r,
      g: leafColor.g,
      b: leafColor.b,
      baseAlpha: leafColor.a,
      veinR: veinColor.r,
      veinG: veinColor.g,
      veinB: veinColor.b,
      veinAlpha: veinColor.a,
      flutterPhase: Math.random() * Math.PI * 2,
      flutterSpeed: Math.random() * 0.012 + 0.004,
      isSinking: false,
      opacity: 1.0
    };
  };

  // Populate initially scattered leaves
  for (let i = 0; i < leafCount; i++) {
    leaves.push(createLeaf(true));
  }

  let time = 0;

  const animate = () => {
    time += 1;

    const rect = canvas.getBoundingClientRect();
    const width = rect.width;
    const height = rect.height;
    const waterHorizon = height * 0.75; // Define water surface horizon (occupying bottom 25% height)

    ctx.clearRect(0, 0, width, height);

    // Decay mouse velocity
    mouse.vx *= 0.94;
    mouse.vy *= 0.94;

    // 1. Update and draw ripples
    ripples = ripples.filter((ripple) => {
      ripple.radius += ripple.speed;
      ripple.alpha = 1 - ripple.radius / ripple.maxRadius;

      if (ripple.alpha <= 0) return false;

      ctx.save();

      // Depth-based parameters: radius scale and Y-axis perspective compression
      const r_scale = 0.3 + 0.7 * ripple.z;
      const perspectiveY = 0.12 + 0.18 * ripple.z; // Flatter at horizon (z=0), rounder at foreground (z=1)

      // Draw 3 concentric wave rings representing a wave packet
      const waveCount = 3;
      const waveSpacing = 10; // distance between consecutive wave crests

      for (let w = 0; w < waveCount; w++) {
        const r = ripple.radius - w * waveSpacing;
        if (r <= 0) continue;

        // Local alpha for this wave ring, decaying as it gets closer to maxRadius
        const localAlpha = (1 - r / ripple.maxRadius) * ripple.alpha;
        if (localAlpha <= 0) continue;

        // Scale radius by depth scale
        const finalRx = r * r_scale;
        const finalRy = r * r_scale * perspectiveY;

        ctx.beginPath();
        // Draw a perfect ellipse on the XZ horizontal plane projected to screen XY
        ctx.ellipse(ripple.x, ripple.y, finalRx, finalRy, 0, 0, Math.PI * 2);

        // Outer ring is brightest, inner rings are softer
        const ringIntensity = w === 0 ? 0.45 : w === 1 ? 0.25 : 0.12;
        ctx.strokeStyle = `rgba(0, 208, 132, ${localAlpha * ringIntensity})`;
        ctx.lineWidth = (w === 0 ? 1.0 : w === 1 ? 0.8 : 0.6) * r_scale;

        if (w === 0) {
          ctx.shadowColor = 'rgba(0, 208, 132, 0.25)';
          ctx.shadowBlur = 4 * r_scale;
        } else {
          ctx.shadowBlur = 0;
        }

        ctx.stroke();
      }

      ctx.restore();
      return true;
    });

    // 2. Update leaves coordinates & states
    leaves.forEach((leaf) => {
      const renderScale = 0.3 + 0.7 * leaf.z;
      const waterY = waterHorizon + leaf.z * (height * 0.25);

      // Water level check (trigger sinking state)
      if (!leaf.isSinking && leaf.y >= waterY) {
        leaf.isSinking = true;
        leaf.vy = 0.08; // Sinking speed
        leaf.vx *= 0.5; // Drag reduction on impact

        // Generate a ripple proportional to leaf size and depth
        ripples.push({
          x: leaf.x,
          y: waterY,
          z: leaf.z,
          radius: 1,
          maxRadius: leaf.size * 3.8,
          alpha: 0.5,
          speed: 0.65
        });
      }

      if (leaf.isSinking) {
        // Liquid friction: drastically slow horizontal movement
        leaf.vx *= 0.93;
        leaf.vy = 0.08;
        leaf.rotationSpeed *= 0.9;

        // Fade out the leaf slowly
        leaf.opacity -= 0.015;

        if (leaf.opacity <= 0) {
          // Respawn at top once faded out
          Object.assign(leaf, createLeaf(false));
        }
      } else {
        // Regular air drift movement
        leaf.vy += (leaf.baseVy - leaf.vy) * 0.04;
        leaf.vx += (leaf.baseVx - leaf.vx) * 0.04;

        // Sway (fluttering) using sine wave
        const sway =
          Math.sin(time * leaf.flutterSpeed + leaf.flutterPhase) * 0.25;
        leaf.vx += sway * 0.15;
        leaf.rotation += leaf.rotationSpeed + sway * 0.003;
        leaf.scaleX = Math.sin(time * leaf.scaleXSpeed + leaf.flutterPhase);

        // Mouse interaction
        const dx = leaf.x - mouse.x;
        const dy = leaf.y - mouse.y;
        const distSq = dx * dx + dy * dy;
        const radius = 150 * renderScale;

        if (distSq < radius * radius) {
          const dist = Math.sqrt(distSq);
          const force = (1 - dist / radius) * renderScale; // 0 to 1

          // Mouse motion drag
          leaf.vx += mouse.vx * force * 0.6;
          leaf.vy += mouse.vy * force * 0.6;

          // Physical repulsion
          const dirX = dx / (dist || 1);
          const dirY = dy / (dist || 1);
          leaf.vx += dirX * force * 0.4;
          leaf.vy += dirY * force * 0.18;
        }

        // Friction
        leaf.vx *= 0.97;
        leaf.vy *= 0.97;

        // Limit speed
        const speedSq = leaf.vx * leaf.vx + leaf.vy * leaf.vy;
        const maxSpeed = 6.5;
        if (speedSq > maxSpeed * maxSpeed) {
          const speed = Math.sqrt(speedSq);
          leaf.vx = (leaf.vx / speed) * maxSpeed;
          leaf.vy = (leaf.vy / speed) * maxSpeed;
        }
      }

      // Apply velocities to coordinates (movement speed matches perspective scale)
      leaf.x += leaf.vx * renderScale;
      leaf.y += leaf.vy * renderScale;

      // Sideways boundary wrap-around
      if (leaf.x < -20) {
        leaf.x = width + 10;
      } else if (leaf.x > width + 20) {
        leaf.x = -10;
      }
    });

    // 3. Draw leaves sorted by depth z (Painters Algorithm for correct 3D occlusion)
    const sortedLeaves = [...leaves].sort((a, b) => a.z - b.z);

    sortedLeaves.forEach((leaf) => {
      const renderScale = 0.3 + 0.7 * leaf.z;

      // Draw leaf
      ctx.save();
      ctx.translate(leaf.x, leaf.y);
      ctx.rotate(leaf.rotation);
      ctx.scale(leaf.scaleX * renderScale, renderScale);

      // Glowing fluorescent shadow effect
      ctx.shadowColor = 'rgba(0, 208, 132, 0.4)';
      ctx.shadowBlur = 5 * renderScale;

      // Draw leaf path
      ctx.beginPath();
      ctx.moveTo(0, -leaf.size);
      ctx.quadraticCurveTo(-leaf.size * 0.7, -leaf.size * 0.2, 0, leaf.size);
      ctx.quadraticCurveTo(leaf.size * 0.7, -leaf.size * 0.2, 0, -leaf.size);
      // Alpha decays slightly with depth to create atmospheric haze
      const depthAlpha = 0.4 + 0.6 * leaf.z;
      ctx.fillStyle = `rgba(${leaf.r}, ${leaf.g}, ${leaf.b}, ${leaf.opacity * leaf.baseAlpha * depthAlpha})`;
      ctx.fill();

      // Draw center vein
      ctx.shadowBlur = 0;
      ctx.beginPath();
      ctx.moveTo(0, -leaf.size);
      ctx.lineTo(0, leaf.size * 0.8);
      ctx.strokeStyle = `rgba(${leaf.veinR}, ${leaf.veinG}, ${leaf.veinB}, ${leaf.opacity * leaf.veinAlpha * depthAlpha})`;
      ctx.lineWidth = 0.8 * renderScale;
      ctx.stroke();

      ctx.restore();
    });

    animationFrameId = requestAnimationFrame(animate);
  };

  animate();

  const handleMouseMove = (e: MouseEvent) => {
    const rect = container.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (lastMousePos) {
      const vx = x - lastMousePos.x;
      const vy = y - lastMousePos.y;

      mouse.vx = vx;
      mouse.vy = vy;
    }

    mouse.x = x;
    mouse.y = y;
    lastMousePos = { x, y };
  };

  const handleMouseLeave = () => {
    mouse.x = -1000;
    mouse.y = -1000;
    mouse.vx = 0;
    mouse.vy = 0;
    lastMousePos = null;
  };

  container.addEventListener('mousemove', handleMouseMove);
  container.addEventListener('mouseleave', handleMouseLeave);
  return () => {
    container.removeEventListener('mousemove', handleMouseMove);
    container.removeEventListener('mouseleave', handleMouseLeave);
    window.removeEventListener('resize', resizeCanvas);
    cancelAnimationFrame(animationFrameId);
  };
}
