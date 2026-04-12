const fs = require('fs');
const path = require('path');

const RECIPES_DIR = '/home/necro/Downloads/AlbionRecipes-main';
const OUTPUT_FILE = '/home/necro/obsidian/assets/curated_recipes.json';

const files = [
  'alchemy.js',
  'hunterslodge.js',
  'magestower.js',
  'warriorsforge.js',
  'cooking.js',
  'butcher.js',
  'saddler.js',
  'toolmaker.js'
];

const allRecipes = [];

files.forEach(file => {
  const fullPath = path.join(RECIPES_DIR, file);
  if (!fs.existsSync(fullPath)) {
    console.warn(`File not found: ${file}`);
    return;
  }

  const content = fs.readFileSync(fullPath, 'utf8');
  
  // Regex to match: new Recipe("ID", new Map([["MAT", QTY], ...]), ..., COUNT)
  // Simplified to capture the ID, the material array string, and the output amount
  const regex = /new\s+Recipe\s*\(\s*"([^"]+)"\s*,\s*new\s+Map\(\s*(\[\[[^\]]+\]\])\s*\)\s*,[^,]*\s*,[^,]*\s*,\s*(\d+)\s*\)/g;
  
  let match;
  while ((match = regex.exec(content)) !== null) {
    const [_, item_id, matArrayStr, amount] = match;
    
    try {
      // Parse [[ "MAT", 8 ], [ "MAT2", 16 ]] style string
      const materials = JSON.parse(matArrayStr).map(([name, count]) => ({
        uniquename: name,
        count: count
      }));

      allRecipes.push({
        uniquename: item_id,
        amount: parseInt(amount),
        materials: materials,
        station: file.replace('.js', '')
      });
    } catch (e) {
      console.error(`Failed to parse materials for ${item_id} in ${file}:`, e.message);
    }
  }
});

fs.writeFileSync(OUTPUT_FILE, JSON.stringify(allRecipes, null, 2));
console.log(`Successfully ingested ${allRecipes.length} recipes into ${OUTPUT_FILE}`);
