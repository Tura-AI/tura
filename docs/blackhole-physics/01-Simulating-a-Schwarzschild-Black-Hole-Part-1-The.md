# Simulating a Schwarzschild Black Hole: Part 1 — The Background and Raytracer | by Caden Marinozzi | Medium

Source: https://medium.com/@cadenmarinozzi/simulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e

[Sitemap](https://medium.com/sitemap/sitemap.xml) [Open in app](https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page---top_nav_layout_nav-----------------------------------------)Sign up

[Sign in](https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&source=post_page---top_nav_layout_nav-----------------------global_nav------------------)

[Medium Logo](https://medium.com/?source=post_page---top_nav_layout_nav-----------------------------------------)Get app[Write](https://medium.com/m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav-----------------------new_post_topnav------------------)[Search](https://medium.com/search?source=post_page---top_nav_layout_nav-----------------------------------------)Sign up

[Sign in](https://medium.com/m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&source=post_page---top_nav_layout_nav-----------------------global_nav------------------)

![Unknown user](https://miro.medium.com/v2/resize:fill:64:64/1*dmbNkD5D-u45r44go_cf0g.png)# Simulating a Schwarzschild Black Hole: Part 1 — The Background and Raytracer

[![Caden Marinozzi](https://miro.medium.com/v2/resize:fill:64:64/1*THtjXN9lM4WdCj9Pf0n4XQ.jpeg)](https://medium.com/@cadenmarinozzi?source=post_page---byline--7de436a56b7e---------------------------------------)[Caden Marinozzi](https://medium.com/@cadenmarinozzi?source=post_page---byline--7de436a56b7e---------------------------------------)9 min read·Jun 2, 2023[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&user=Caden+Marinozzi&userId=6fde3cc6c65f&source=---header_actions--7de436a56b7e---------------------clap_footer------------------)--

[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&user=Caden+Marinozzi&userId=6fde3cc6c65f&source=---header_actions--7de436a56b7e---------------------repost_header------------------)[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&source=---header_actions--7de436a56b7e---------------------bookmark_footer------------------)[Listen

](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3D7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&source=---header_actions--7de436a56b7e---------------------post_audio_button------------------)Share

After watching the film *Interstellar*, many of us are left with a sense of awe due to the black hole, Gargantua, which is shown in the movie.

Press enter or click to view image in full sizeThe Gargantua black hole from InterstellarI too was in awe, and after many sleepless nights of research, I decided to create my own, and now you can too!

## What is a Black Hole?

Not so simply, a black hole is a singularity in space so dense that the escape velocity required to withdraw from its gravitational curvature of space-time is greater than the speed of light. Or simply: Something with enough mass in such a small space that even light cannot escape it.

## Setting up the project

Before we do anything else, let’s set up the project itself. This article will show the development of the project using web graphics and JavaScript/HTML, however, this process can be followed on any platform that supports graphical rendering using the GLSL programming language.

### index.html

Start off by creating a new directory and an “index.html” file to go with it:

```
<!DOCTYPE html><html>  <head></head>  <body></body></html>
```

By default, all elements have a margin set, we’ll remove it and set the background to black using an in-html stylesheet:

```
<!-- After "<head> and before </head>" --><style>  * {      padding: 0;      margin: 0;      background: black;  }</style>
```

### Three.js

The rendering engine I will be using is Three.js as it easily allows textures and objects to be created without any hassle of dealing with vertices and all of that. We will need to import it using an import map:

```
<!-- After "</style> and before </head>" --><script type="importmap">  {    "imports": {      "three": "https://unpkg.com/three/build/three.module.js"    }  }</script>
```

### script.js

We need a JavaScript file to run the code, we’ll call it “script.js”. Create it in the same directory. To run it with our HTML, add it as a module script in our body element:

```
<!-- After "<body> and before </body>" --><script type='module' src="script.js"></script>
```

Your index.html file should now look like this:

```
<!DOCTYPE html><html>  <head>        <style>            * {                padding: 0;                margin: 0;                        background: black;            }        </style>        <script type="importmap">        {            "imports": {              "three": "https://unpkg.com/three/build/three.module.js"            }        }        </script>    </head>    <body>        <script type='module' src="script.js"></script>    </body></html>
```

## Setting up Three.js

In our script.js file, we need to initialize a Three.js renderer to render our black hole.

Import Three.js and create a scene:

```
import * as THREE from "three";const scene = new THREE.Scene();
```

Next, we’ll need a camera and renderer:

```
const width = window.innerWidth; // These can be changedconst height = window.innerHeight; // These can be changedconst aspectRatio = width / height;const camera = new THREE.PerspectiveCamera(75, aspectRatio, 0.1, 1000);camera.position.z = 1;const renderer = new THREE.WebGLRenderer({  antialias: true,});renderer.setSize(width, height);renderer.setPixelRatio(window.devicePixelRatio);document.body.appendChild(renderer.domElement);
```

The larger the width and height, the more pixels will need to be rendered, and the slower rendering will be.

### The canvas

Since we are using a 3D renderer but rendering a 2D image of a 3D scene processed in 2D (confusing, I know!) we will need to create a 2D canvas inside of our 3D scene to display the rendering on. The math for it is weird and you don’t need to understand it, but basically, we are stretching a 2D plane to fit in the dimensions of the screen, essentially creating a new canvas in the 3D world.

```
// Converts degrees to radiansfunction degToRad(deg) {  return (deg * Math.PI) / 180;}const fovRadians = degToRad(camera.fov);const yFov = camera.position.z * Math.tan(fovRadians / 2) * 2;const canvasGeometry = new THREE.PlaneGeometry(yFov * camera.aspect, yFov);
```

The next step is to create our shader material, as we will use GLSL to program the black hole visualization. For now, we will keep the shader properties empty.

```
const canvasMaterial = new THREE.ShaderMaterial({  uniforms: {},  vertexShader,  fragmentShader,});
```

Then we will create a mesh from the geometry and material, and add it to the scene:

```
const canvasMesh = new THREE.Mesh(canvasGeometry, canvasMaterial);scene.add(canvasMesh);
```

The last step in setting up Three.js is rendering the scene from the camera:

```
renderer.render(scene, camera);
```

## Setting up the shaders

There are two shaders in a shader material, the vertex shader, and the fragment shader. The vertex shader handles the positioning of pixels and the fragment shader handles the color of pixls. To make it easier, we’ll separate these two shaders from the rest of the code using imports. Create two files, one called “vertexShader.js”, and the other called “fragmentShader.js”. Add the files to the import map like so:

```
// Replace the current importmap with this new one{  "imports": {    "three": "https://unpkg.com/three/build/three.module.js",    "vertexShader": "./vertexShader.js", // <--     "fragmentShader": "./fragmentShader.js" // <--   }}
```

Then import them into the script:

```
// After import * as THREE from "three";import vertexShader from "vertexShader";import fragmentShader from "fragmentShader";
```

Now we are ready to start developing the shaders!

## Vertex Shader

The vertex shader is pretty simple and doesn’t need to be changed at all from a standard vertex shader, besides exposing the UV coordinate to the fragment shader. Since it’s defined in a JavaScript file, we’ll need to export it as a default string:

```
export default `    varying vec2 vUv;    void main() {        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);        vUv = uv;    }`;
```

We expose the uv coordinate as a varying vec2 which allows us to access it from the fragment shader.

## Fragment Shader

Let’s create the base of the fragment shader before going further:

```
export default `  varying vec2 vUv;    void main() {    gl_FragColor = vec4(vUv.x, vUv.y, 0.0, 1.0);  }`;
```

This sets the red and green values of the pixel to the X and Y coordinate of the UV.

If all went well you should see a gradient filling the screen:

Press enter or click to view image in full sizeA gradient of the UV coordinates of the canvas## The three main structures of a black hole

There are three main structures of a black hole, the event horizon, the warped background, and the accretion disk. To begin with, let’s focus on just the warped background.

### Warped Background

The warped background is the distorted space behind a black hole. This occurs because of the curved path light takes while propagating toward the viewer around a black hole.

Press enter or click to view image in full sizeA distorted galaxy around a black hole## Creating the warped background

Let’s start by adding a space texture to our scene.

I am using an [8k space image](https://upload.wikimedia.org/wikipedia/commons/8/85/Solarsystemscope_texture_8k_stars_milky_way.jpg) from Wikimedia, however, you may use whatever background texture you want, and the process is the same.

Remove the original **renderer.render(scene, camera)** statement and insert this new statement that will load the texture and render the scene once it has been loaded:

```
// After const canvasGeometry = new THREE.PlaneGeometry(yFov * camera.aspect, yFov);const spaceTexture = new THREE.TextureLoader().load("space texture file here", () =>  renderer.render(scene, camera));
```

Replace “space texture file here” with the path to your texture file.

Then, add the texture as a uniform to the shader material:

```
// Replace the current uniforms with this oneuniforms: {    uSpaceTexture: {      value: spaceTexture,    },  },
```

In the fragment shader, add the texture as a uniform sampler2D:

```
// After varying vec2 vUv;uniform sampler2D uSpaceTexture;
```

And replace the current **gl_FragColor** statement with a new one that sets the color to a sample of the texture at the current UV coordinate.

```
// Replace gl_FragColor = texture2D(uSpaceTexture, vUv);gl_FragColor = texture2D(uSpaceTexture, vUv);
```

Press enter or click to view image in full sizeThe space texture mapped onto the canvasYou should be able to see your texture mapped onto the canvas.

### Raytracing

The simulation will work by propagating a ray toward the black hole and coloring the pixel it reaches at the end of its iteration according to what the ray hits. Let’s create the raytracer.

Ray tracing works by changing the position of a ray from its starting position by a fixed step size. So to do this we will need an observer position, which will be the camera, and a target position, which will be the black hole. Then for a certain number of iterations, we will step the ray. Once the iterations have been completed, we will color the ray accordingly.

A diagram of fixed-step ray tracingFirst, let’s scale the UV coordinate to the resolution of the screen. As the aspect ratio of the screen is usually not 1:1.

In the main script, add a new uniform that will hold the resolution of the screen:

```
/*AfteruSpaceTexture: {      value: spaceTexture,},*/uResolution: {  value: new THREE.Vector2(width, height),},
```

Add the uniform to the fragment shader:

```
// After uniform sampler2D uSpaceTexture;uniform vec2 uResolution;
```

Add the following code to the main method in the fragment shader:

```
vUv = (vUv - 0.5) * 2.0 * vec2(uResolution.x / uResolution.y, 1);
```

The math behind it basically shifts the UV to the center of the screen, scales it down, and then scales its X dimension by the ratio between the X and Y resolution.

Now, let’s define some constants that we will use in the code.

```
// After uniform vec2 uResolution;#define MAX_ITERATIONS 200#define STEP_SIZE 0.1
```

The higher the iterations, the slower the simulation will be, as it will propagate for longer. The step size and max iterations may be inversely proportional to each other to improve accuracy, however, if the step size is too small or the iterations are too small, detail may be lost.

Next, we’ll define a position for the camera and black hole:

```
// After #define STEP_SIZE 0.1vec3 camPos = vec3(0, 0, -10);vec3 blackholePos = vec3(0, 0, 0);
```

The black hole is located at the center of the scene, and the camera is located 10 units behind the black hole.

In our main method, we’ll define a direction and start position for the ray. The ray should propagate towards the current pixel which is defined by the UV coordinate, however, we will normalize it because we don’t care about the magnitude. We also want the ray to start at the camera’s position.

```
// After vec2 uv = (vUv - 0.5) * 2.0 * vec2(uResolution.x / uResolution.y, 1);vec3 rayDir = normalize(vec3(uv, 1));vec3 rayPos = camPos;
```

Next, we’ll create a method to ray trace from the ray’s position in the ray’s direction. Let’s call it “raytrace”, and we’ll have it return a vec4 containing the RGBA value of the pixel. We’ll have it take in the position and direction of the ray.

```
// After vec3 blackholePos = vec3(0, 0, 0);vec4 raytrace(vec3 rayDir, vec3 rayPos) {}
```

We’ll start by defining the base color:

```
// After vec4 raytrace(vec3 rayDir, vec3 rayPos) {vec4 color = vec4(0, 0, 0, 1);
```

Now we will need to create a loop that will step the ray for the number of iterations we defined, and for each iteration, we will step the. position of the ray in the direction of the ray:

```
// After vec4 color = vec4(0, 0, 0, 1);for (int i = 0; i < MAX_ITERATIONS; i++) {  rayPos += rayDir * STEP_SIZE;}
```

And then we’ll sample the texture from the direction of the ray and return the color:

```
/*After  rayPos += rayDir * STEP_SIZE;}*/color = texture2D(uSpaceTexture, rayDir.xy);return color;
```

You should still see the flat background texture, but now it will be sampled from the ray direction.

### Schwarzschild Metric

Press enter or click to view image in full sizeThe Schwarzschild MetricThe Schwarzschild Metric is an equation that defines the distance between two events in curved spacetime, **ds**. Numerically integrating the equation requires getting the individual components of the resulting vector. After doing this process we get a single equation that we can use to step the direction of the ray. We will be using Euler’s method to step the ray:

Eulers methodPress enter or click to view image in full sizeThe numerical integration equations of the Schwarzschild MetricThese may seem complicated but the corresponding code is quite simple:

```
float dist = length(rayPos - blackholePos);float h2 = pow(length(cross(rayPos, rayDir)), 2.0);rayDir += -1.5 * h2 * rayPos / pow(pow(dist, 2.0), 2.5) * STEP_SIZE;
```

First, calculate **h2** outside of the raytracing loop:

```
// After vec4 color = vec4(0, 0, 0, 1);float h2 = pow(length(cross(rayPos, rayDir)), 2.0);
```

Then calculate the distance between the ray and the black hole, and update the ray direction:

```
// After for (int i = 0; i < MAX_ITERATIONS; i++) {float dist = length(rayPos - blackholePos);rayDir += -1.5 * h2 * rayPos / pow(pow(dist, 2.0), 2.5) * STEP_SIZE;
```

We have now successfully implemented gravitational curvature to our raytracer. Running the code should show a warped image of the background texture:

Press enter or click to view image in full sizeA gravitationally warped space texture[Black Hole](https://medium.com/tag/black-holes?source=post_page-----7de436a56b7e---------------------------------------)[Raytracing](https://medium.com/tag/ray-tracing?source=post_page-----7de436a56b7e---------------------------------------)[Schwarzschild](https://medium.com/tag/schwarzschild?source=post_page-----7de436a56b7e---------------------------------------)[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&user=Caden+Marinozzi&userId=6fde3cc6c65f&source=---footer_actions--7de436a56b7e---------------------clap_footer------------------)--

[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&user=Caden+Marinozzi&userId=6fde3cc6c65f&source=---footer_actions--7de436a56b7e---------------------clap_footer------------------)--

[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&user=Caden+Marinozzi&userId=6fde3cc6c65f&source=---footer_actions--7de436a56b7e---------------------repost_footer------------------)[](https://medium.com/m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2F7de436a56b7e&operation=register&redirect=https%3A%2F%2Fmedium.com%2F%40cadenmarinozzi%2Fsimulating-a-schwarzschild-black-hole-part-1-the-background-and-raytracer-7de436a56b7e&source=---footer_actions--7de436a56b7e---------------------bookmark_footer------------------)[![Caden Marinozzi](https://miro.medium.com/v2/resize:fill:96:96/1*THtjXN9lM4WdCj9Pf0n4XQ.jpeg)](https://medium.com/@cadenmarinozzi?source=post_page---post_author_info--7de436a56b7e---------------------------------------)[![Caden Marinozzi](https://miro.medium.com/v2/resize:fill:128:128/1*THtjXN9lM4WdCj9Pf0n4XQ.jpeg)](https://medium.com/@cadenmarinozzi?source=post_page---post_author_info--7de436a56b7e---------------------------------------)[## Written by Caden Marinozzi

](https://medium.com/@cadenmarinozzi?source=post_page---post_author_info--7de436a56b7e---------------------------------------)[5 followers](https://medium.com/@cadenmarinozzi/followers?source=post_page---post_author_info--7de436a56b7e---------------------------------------)·[2 following](https://medium.com/@cadenmarinozzi/following?source=post_page---post_author_info--7de436a56b7e---------------------------------------)Software engineer and aspiring astrophysicist.

[Help

](https://help.medium.com/hc/en-us?source=post_page-----7de436a56b7e---------------------------------------)[Status

](https://status.medium.com/?source=post_page-----7de436a56b7e---------------------------------------)[About

](https://medium.com/about?autoplay=1&source=post_page-----7de436a56b7e---------------------------------------)[Careers

](https://medium.com/jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----7de436a56b7e---------------------------------------)[Press

](https://medium.com/@cadenmarinozzi/mailto:pressinquiries@medium.com)[Blog

](https://blog.medium.com/?source=post_page-----7de436a56b7e---------------------------------------)[Store

](https://medium.com/store)[Privacy

](https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----7de436a56b7e---------------------------------------)[Rules

](https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----7de436a56b7e---------------------------------------)[Terms

](https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----7de436a56b7e---------------------------------------)[Text to speech

](https://speechify.com/medium?source=post_page-----7de436a56b7e---------------------------------------)

## Media links

- <https://miro.medium.com/v2/resize:fill:1000:1000/7*GAOKVe--MXbEJmV9230oOQ.png>
- <https://miro.medium.com/v2/resize:fill:64:64/1*dmbNkD5D-u45r44go_cf0g.png>
- <https://miro.medium.com/v2/resize:fill:64:64/1*THtjXN9lM4WdCj9Pf0n4XQ.jpeg>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:1400/format:webp/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:640/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:720/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:750/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:786/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:828/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:1100/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:1400/1*8hxIU3mZPHUDY-zfSOa06w.png>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:1400/format:webp/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:640/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:720/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:750/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:786/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:828/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:1100/0*9X6GTdL4jp86TCgO.jpg>
- <https://miro.medium.com/v2/resize:fit:1400/0*9X6GTdL4jp86TCgO.jpg>
- <https://upload.wikimedia.org/wikipedia/commons/8/85/Solarsystemscope_texture_8k_stars_milky_way.jpg>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:1400/format:webp/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:640/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:720/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:750/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:786/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:828/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:1100/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:1400/1*mdj9MRK2NKqwiY7zhVLTow.png>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:1370/format:webp/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:640/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:720/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:750/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:786/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:828/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:1100/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:1370/0*GQfQWPrHzLaM8jPN.gif>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:1400/format:webp/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:640/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:720/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:750/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:786/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:828/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:1100/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:1400/0*nUDcJpxBhH1FOUCQ.png>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:634/format:webp/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:640/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:720/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:750/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:786/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:828/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:1100/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:634/1*eHvSlAOmWrcmoRbj0zI3ow.png>
- <https://miro.medium.com/v2/resize:fit:640/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:720/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:750/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:786/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:828/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:1100/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:1400/format:webp/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:640/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:720/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:750/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:786/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:828/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:1100/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fit:1400/1*kfqXCZLy63WwBXVxN8B2wA.png>
- <https://miro.medium.com/v2/resize:fill:96:96/1*THtjXN9lM4WdCj9Pf0n4XQ.jpeg>
- <https://miro.medium.com/v2/resize:fill:128:128/1*THtjXN9lM4WdCj9Pf0n4XQ.jpeg>
- <https://miro.medium.com/v2/resize:fit:500/7%2AV1_7XP4snlmqrc_0Njontw.png>
