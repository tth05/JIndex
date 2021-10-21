package com.github.tth05.jindex.entries;

import com.github.tth05.jindex.constantPool.ConstantPool;
import com.github.tth05.jindex.constantPool.IndexString;

import java.util.Arrays;

public class IndexedPackage extends ConstantPoolReference {


    private IndexedPackage[] subPackages;
    private final IndexedPackage previousPackage;

    public IndexedPackage(int cpNameIndex, IndexedPackage previousPackage) {
        super(cpNameIndex);
        this.previousPackage = previousPackage;
    }

    public IndexedPackage getOrAddPackage(ConstantPool constantPool, IndexString name) {
        int dotIndex = name.indexOf((byte) '.');
        IndexString subName;
        if (dotIndex == -1) {
            if (getName(constantPool).equals(name))
                return this;
            subName = name;
        } else {
            subName = name.subSequence(0, dotIndex);
        }

        IndexedPackage existingPackage = null;
        if (this.subPackages != null) {
            for (IndexedPackage subPackage : this.subPackages) {
                if (constantPool.stringAt(subPackage.cpNameIndex).equals(subName)) {
                    existingPackage = subPackage;
                    break;
                }
            }
        }

        if (existingPackage == null) {
            existingPackage = new IndexedPackage(constantPool.addString(subName), this);
            addPackage(existingPackage);
        }
        return dotIndex == -1 ? existingPackage : existingPackage.getOrAddPackage(constantPool, name.subSequence(dotIndex + 1, name.length()));
    }

    public void addPackage(IndexedPackage indexedPackage) {
        if (this.subPackages == null) {
            this.subPackages = new IndexedPackage[]{indexedPackage};
            return;
        }

        this.subPackages = Arrays.copyOf(this.subPackages, this.subPackages.length + 1);
        this.subPackages[this.subPackages.length - 1] = indexedPackage;
    }

    public String getFullName(ConstantPool constantPool) {
        StringBuilder builder = new StringBuilder(this.getName(constantPool).toJavaString());
        IndexedPackage previous = this;
        while ((previous = previous.previousPackage) != null) {
            String str = previous.toJavaString(constantPool);
            builder.insert(0, str).insert(str.length(), '.');
        }

        return builder.toString();
    }

    public void freeSubPackages() {
        this.subPackages = null;
    }

    public IndexedPackage[] getSubPackages() {
        if (this.subPackages == null)
            return null;
        return Arrays.copyOf(this.subPackages, this.subPackages.length);
    }

    @Override
    public String toString() {
        return "IndexedPackage{cp: '" + this.cpNameIndex + "'}";
    }
}
