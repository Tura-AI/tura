const currency = new Intl.NumberFormat("ja-JP", {
  style: "currency",
  currency: "JPY",
  maximumFractionDigits: 0,
});

const products = [
  {
    id: "mat-sui",
    name: "Sui Grip Mat 4.5mm",
    category: "mat",
    label: "best seller",
    price: 16800,
    image: "../../assets/yoga-jp-store/media/generate-media-replicate_z_image_turbo-1.png",
    summary: "天然ラバーの密着感と、畳一枚分に収まる落ち着いた色設計。",
    detail: "表面は湿度に強い微細グリップ。自宅練習からスタジオまで、床鳴りを抑える密度で仕上げています。",
    choices: ["Sage", "Charcoal", "Stone"],
  },
  {
    id: "wear-kiri",
    name: "Kiri Layer Set",
    category: "wear",
    label: "new",
    price: 22400,
    image: "../../assets/yoga-jp-store/media/generate-media-gemini_3_1_flash-1.jpg",
    summary: "朝のクラスから街まで崩れない、炭灰と生成りのレイヤー。",
    detail: "薄手ながら透けにくい編地。長時間座っても膝裏が重くならないよう、縫い目を最小限にしました。",
    choices: ["S", "M", "L"],
  },
  {
    id: "prop-koishi",
    name: "Koishi Prop Kit",
    category: "care",
    label: "studio kit",
    price: 9800,
    image: "../../assets/yoga-jp-store/media/generate-media-gemini_3_1_flash-1-1.jpg",
    summary: "コルクブロック、コットンストラップ、リネンタオルの基本セット。",
    detail: "支える、伸ばす、拭う。派手さはありませんが、練習の質を静かに底上げする道具だけを揃えています。",
    choices: ["Natural", "Warm grey"],
  },
  {
    id: "care-shiro",
    name: "Shiro Care Bottle",
    category: "care",
    label: "daily care",
    price: 6200,
    image: "../../assets/yoga-jp-store/media/generate-media-replicate_z_image_turbo-1-2.png",
    summary: "ステンレスボトル、タオル、ストラップの通勤前ケアセット。",
    detail: "水分補給とマットケアを一つに。バッグの中で主張しすぎない、白磁のようなマット仕上げです。",
    choices: ["White", "Sand", "Mist"],
  },
];

const plan = {
  id: "plan-care-refill",
  name: "Care Refill 定期便",
  category: "care",
  label: "monthly",
  price: 4900,
  image: "../../assets/yoga-jp-store/media/generate-media-replicate_z_image_turbo-1-2.png",
  summary: "マットスプレー、コットンタオル、リネンバッグの月額便。",
  detail: "必要な月だけスキップできます。",
  choices: ["Monthly"],
};

const state = {
  filter: "all",
  sort: "featured",
  cart: new Map(),
};

const productGrid = document.querySelector("[data-products]");
const detailRoot = document.querySelector("[data-product-detail]");
const productDrawer = document.querySelector('[data-drawer="product"]');
const cartDrawer = document.querySelector('[data-drawer="cart"]');
const cartItems = document.querySelector("[data-cart-items]");

function formatPrice(value) {
  return currency.format(value);
}

function visibleProducts() {
  const filtered = state.filter === "all" ? products : products.filter((product) => product.category === state.filter);
  return [...filtered].sort((a, b) => {
    if (state.sort === "price-asc") return a.price - b.price;
    if (state.sort === "price-desc") return b.price - a.price;
    return products.findIndex((item) => item.id === a.id) - products.findIndex((item) => item.id === b.id);
  });
}

function renderProducts() {
  productGrid.innerHTML = visibleProducts()
    .map(
      (product) => `
        <article class="product-tile">
          <button class="product-media" type="button" data-view-product="${product.id}" aria-label="${product.name} の詳細を見る">
            <img src="${product.image}" alt="${product.name}" loading="lazy" />
            <span class="badge">${product.label}</span>
          </button>
          <div class="product-info">
            <div>
              <h3>${product.name}</h3>
              <p>${product.summary}</p>
            </div>
            <span class="price">${formatPrice(product.price)}</span>
          </div>
          <button class="quick-button" type="button" data-add-product="${product.id}">バッグへ追加</button>
        </article>
      `,
    )
    .join("");
}

function renderDetail(product) {
  detailRoot.innerHTML = `
    <div class="detail-media">
      <img src="${product.image}" alt="${product.name}" />
    </div>
    <p class="detail-meta">${product.label} / 税込価格</p>
    <h2 id="drawer-title">${product.name}</h2>
    <p>${product.detail}</p>
    <div class="detail-price">${formatPrice(product.price)}</div>
    <div class="choice-row" aria-label="選択肢">
      ${product.choices
        .map((choice, index) => `<button class="choice ${index === 0 ? "is-selected" : ""}" type="button">${choice}</button>`)
        .join("")}
    </div>
    <button class="checkout-button" type="button" data-add-product="${product.id}">バッグへ追加</button>
  `;
}

function openDrawer(drawer) {
  drawer.classList.add("is-open");
  drawer.setAttribute("aria-hidden", "false");
  document.body.style.overflow = "hidden";
}

function closeDrawer(drawer) {
  drawer.classList.remove("is-open");
  drawer.setAttribute("aria-hidden", "true");
  if (!document.querySelector(".drawer.is-open")) {
    document.body.style.overflow = "";
  }
}

function findProduct(id) {
  return [...products, plan].find((product) => product.id === id);
}

function addToCart(id) {
  const product = findProduct(id);
  if (!product) return;
  const current = state.cart.get(id) ?? { product, qty: 0 };
  current.qty += 1;
  state.cart.set(id, current);
  renderCart();
  openDrawer(cartDrawer);
}

function updateQty(id, delta) {
  const item = state.cart.get(id);
  if (!item) return;
  item.qty += delta;
  if (item.qty <= 0) {
    state.cart.delete(id);
  } else {
    state.cart.set(id, item);
  }
  renderCart();
}

function cartTotals() {
  const subtotal = [...state.cart.values()].reduce((sum, item) => sum + item.product.price * item.qty, 0);
  const shipping = subtotal === 0 || subtotal >= 18000 ? 0 : 800;
  return { subtotal, shipping, total: subtotal + shipping };
}

function renderCart() {
  const items = [...state.cart.values()];
  document.querySelector("[data-cart-count]").textContent = String(items.reduce((sum, item) => sum + item.qty, 0));
  cartItems.innerHTML = items.length
    ? items
        .map(
          ({ product, qty }) => `
            <article class="cart-item">
              <div class="cart-thumb"><img src="${product.image}" alt="${product.name}" /></div>
              <div>
                <h3>${product.name}</h3>
                <p>${product.summary}</p>
                <div class="qty-control" aria-label="数量変更">
                  <button type="button" data-qty="${product.id}" data-delta="-1" aria-label="${product.name} を減らす">−</button>
                  <span>${qty}</span>
                  <button type="button" data-qty="${product.id}" data-delta="1" aria-label="${product.name} を増やす">＋</button>
                </div>
              </div>
              <span class="price">${formatPrice(product.price * qty)}</span>
            </article>
          `,
        )
        .join("")
    : '<p class="empty-cart">バッグは空です。</p>';

  const totals = cartTotals();
  document.querySelector("[data-cart-subtotal]").textContent = formatPrice(totals.subtotal);
  document.querySelector("[data-cart-shipping]").textContent = totals.shipping === 0 ? "無料" : formatPrice(totals.shipping);
  document.querySelector("[data-cart-total]").textContent = formatPrice(totals.total);
}

document.addEventListener("click", (event) => {
  const target = event.target.closest("button, a, [data-close-cart], [data-close-drawer]");
  if (!target) return;

  const productId = target.dataset.viewProduct;
  if (productId) {
    const product = findProduct(productId);
    renderDetail(product);
    openDrawer(productDrawer);
    return;
  }

  if (target.dataset.addProduct) {
    addToCart(target.dataset.addProduct);
    return;
  }

  if (target.dataset.filter) {
    state.filter = target.dataset.filter;
    document.querySelectorAll("[data-filter]").forEach((button) => {
      const active = button.dataset.filter === state.filter;
      button.classList.toggle("is-active", active);
      button.setAttribute("aria-selected", String(active));
    });
    renderProducts();
    return;
  }

  if (target.dataset.openCart !== undefined) {
    openDrawer(cartDrawer);
    return;
  }

  if (target.dataset.closeDrawer !== undefined) {
    closeDrawer(productDrawer);
    return;
  }

  if (target.dataset.closeCart !== undefined) {
    closeDrawer(cartDrawer);
    return;
  }

  if (target.dataset.addPlan !== undefined) {
    addToCart(plan.id);
    return;
  }

  if (target.dataset.qty) {
    updateQty(target.dataset.qty, Number(target.dataset.delta));
    return;
  }

  if (target.classList.contains("choice")) {
    target.parentElement.querySelectorAll(".choice").forEach((choice) => choice.classList.remove("is-selected"));
    target.classList.add("is-selected");
  }
});

document.querySelector("[data-sort]").addEventListener("change", (event) => {
  state.sort = event.target.value;
  renderProducts();
});

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  closeDrawer(productDrawer);
  closeDrawer(cartDrawer);
});

renderProducts();
renderCart();
